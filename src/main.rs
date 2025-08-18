
mod instruments;

use anyhow::Result;
use axum::{
    extract::{Path, State},
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dotenvy::dotenv;
use futures::StreamExt;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, VecDeque}, net::SocketAddr, str::FromStr, time::Duration};
use axum::extract::ws::Utf8Bytes;
use tokio::{net::TcpListener, time::sleep};
use tokio_tungstenite::connect_async;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

// ====================== server config ======================
#[derive(Clone)]
struct ServerCfg {
    bind_addr: SocketAddr,
    history_capacity: usize,
    // default streams to start on boot
    streams: Vec<StreamCfg>,
    // redis url for API to read cache
    redis_url: String,
}

impl ServerCfg {
    fn from_env() -> Self {
        use std::env;
        let get = |k:&str, d:&str| env::var(k).unwrap_or_else(|_| d.to_string());
        let parse_usize = |k:&str, d:usize| env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        Self {
            bind_addr: get("BIND_ADDR", "0.0.0.0:8787").parse().unwrap(),
            history_capacity: parse_usize("HISTORY_CAPACITY", 2000),
            streams: vec![
                StreamCfg { exchange: Exchange::BinanceSpot, symbol: "BTCUSDT".into(), interval: "1m".into() },
                StreamCfg { exchange: Exchange::BinanceSpot, symbol: "ETHUSDT".into(), interval: "1m".into() },
            ],
            redis_url: get("REDIS_URL", "redis://127.0.0.1/"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Exchange { BinanceSpot, BinanceUSDM }
impl Exchange {
    fn as_str(&self)->&'static str { match self { Self::BinanceSpot=>"binance_spot", Self::BinanceUSDM=>"binance_um" } }
    fn ws_base(&self)->&'static str{ match self { Self::BinanceSpot=>"wss://stream.binance.com:9443", Self::BinanceUSDM=>"wss://fstream.binance.com" } }
}
impl FromStr for Exchange {
    type Err = anyhow::Error;
    fn from_str(s:&str)->Result<Self> {
        match s { "binance_spot"=>Ok(Self::BinanceSpot), "binance_um"=>Ok(Self::BinanceUSDM), _=>Err(anyhow::anyhow!("unsupported exchange {s}")) }
    }
}
#[derive(Clone, Debug)]
struct StreamCfg { exchange: Exchange, symbol:String, interval:String }
impl StreamCfg {
    fn key(&self)->String { format!("{}:{}:{}", self.exchange.as_str(), self.symbol, self.interval) }
    fn ws_url(&self)->String { format!("{}/ws/{}@kline_{}", self.exchange.ws_base(), self.symbol.to_lowercase(), self.interval) }
}

// ====================== domain ======================
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Candle { t:u64, ct:u64, o:f64, h:f64, l:f64, c:f64, v:f64, final_bar:bool }

#[derive(Clone, Debug, Serialize)]
#[serde(tag="type", content="data")]
enum OutMsg {
    Snapshot{ candles: Vec<Candle> },
    Append{ candle: Candle },
    UpdateLast{ candle: Candle },
}

#[derive(Deserialize)]
struct BinanceKlineEvent { e:String, E:u64, s:String, k:BinanceKline }
#[derive(Deserialize)]
struct BinanceKline { t:u64, T:u64, o:String, h:String, l:String, c:String, v:String, x:bool }

// ====================== shared state ======================
#[derive(Clone)]
struct Hub(std::sync::Arc<RwLock<HashMap<String, tokio::sync::broadcast::Sender<OutMsg>>>>);
impl Hub {
    fn new()->Self{ Self(std::sync::Arc::new(RwLock::new(HashMap::new()))) }
    fn get_or_create(&self, k:&str)->tokio::sync::broadcast::Sender<OutMsg>{
        let mut m=self.0.write(); m.entry(k.to_string()).or_insert_with(|| tokio::sync::broadcast::channel(1024).0).clone()
    }
    fn get(&self, k:&str)->Option<tokio::sync::broadcast::Sender<OutMsg>>{ self.0.read().get(k).cloned() }
}
#[derive(Clone)]
struct Book(std::sync::Arc<RwLock<HashMap<String, VecDeque<Candle>>>>);
impl Book {
    fn new()->Self{ Self(std::sync::Arc::new(RwLock::new(HashMap::new()))) }
    fn push_or_update(&self, key:&str, cap:usize, c:Candle)->(bool, Candle){
        let mut m=self.0.write();
        let ring = m.entry(key.to_string()).or_insert_with(|| VecDeque::with_capacity(cap));
        let is_append = if let Some(last)=ring.back_mut() { if last.t==c.t { *last=c; false } else { ring.push_back(c); true } } else { ring.push_back(c); true };
        while ring.len()>cap { ring.pop_front(); }
        (is_append, *ring.back().unwrap())
    }
    fn snapshot(&self, key:&str)->Vec<Candle>{
        self.0.read().get(key).map(|r| r.iter().copied().collect()).unwrap_or_default()
    }
}

// ====================== server ctx ======================
#[derive(Clone)]
struct AppCtx { cfg: ServerCfg, hub: Hub, book: Book, instr_cfg: instruments::InstrumentsCfg }

// ====================== consumers ======================
async fn run_consumer(ctx:AppCtx, sc:StreamCfg)->Result<()>{
    let key=sc.key();
    loop {
        let url=sc.ws_url();
        info!("connect [{key}] {url}");
        match connect_async(&url).await {
            Ok((ws,_))=>{
                let (_, mut read)=ws.split();
                let _ = ctx.hub.get_or_create(&key);
                while let Some(msg)=read.next().await {
                    match msg {
                        Ok(tokio_tungstenite::tungstenite::Message::Text(txt))=>{
                            if let Err(e)=handle_msg(&ctx,&sc,&key,&txt).await { error!("handle_msg {key} {e:?}"); }
                        }
                        Ok(tokio_tungstenite::tungstenite::Message::Close(_))=>{ break; }
                        Ok(_) => {}
                        Err(e)=>{ error!("ws read {key} {e:?}"); break; }
                    }
                }
            }
            Err(e)=>{ error!("connect {key} {e:?}"); }
        }
        sleep(Duration::from_secs(3)).await;
    }
}

async fn handle_msg(ctx:&AppCtx, sc:&StreamCfg, key:&str, txt:&str)->Result<()>{
    let v:serde_json::Value = match serde_json::from_str(txt){ Ok(v)=>v, Err(_)=>return Ok(()) };
    let ev:BinanceKlineEvent = if v.get("k").is_some(){ serde_json::from_value(v)? } else if let Some(d)=v.get("data"){ if d.get("k").is_some(){ serde_json::from_value(d.clone())? } else { return Ok(()) } } else { return Ok(()) };
    if !ev.s.eq_ignore_ascii_case(&sc.symbol){ return Ok(()); }
    let c=Candle{ t:ev.k.t, ct:ev.k.T, o:ev.k.o.parse()?, h:ev.k.h.parse()?, l:ev.k.l.parse()?, c:ev.k.c.parse()?, v:ev.k.v.parse()?, final_bar:ev.k.x };
    let (om, _)={
        let (is_append,last)=ctx.book.push_or_update(key, ctx.cfg.history_capacity, c);
        let m = if is_append { OutMsg::Append{ candle:last } } else { OutMsg::UpdateLast{ candle:last } };
        (m,is_append)
    };
    if let Some(tx)=ctx.hub.get(key) { let _ = tx.send(om); }
    Ok(())
}

// ====================== HTTP handlers ======================
#[derive(Deserialize)]
struct WsPath{ exchange:String, symbol:String, interval:String }
async fn ws_handler(ws:WebSocketUpgrade, State(ctx):State<AppCtx>, Path(WsPath{exchange,symbol,interval}):Path<WsPath>) -> impl IntoResponse {
    let key=format!("{exchange}:{symbol}:{interval}");
    ws.on_upgrade(move |sock| client_ws(sock, ctx, key))
}
async fn client_ws(mut socket:WebSocket, ctx:AppCtx, key:String){
    let tx=ctx.hub.get_or_create(&key);
    let mut rx=tx.subscribe();
    let snap=ctx.book.snapshot(&key);
    let _=socket.send(Message::Text(Utf8Bytes::from(serde_json::to_string(&OutMsg::Snapshot { candles: snap }).unwrap()))).await;
    loop {
        tokio::select!{
            Ok(m)=rx.recv()=> {
                if socket.send(Message::Text(Utf8Bytes::from(serde_json::to_string(&m).unwrap()))).await.is_err(){ break; }
            }
        }
    }
}

// GET /api/instruments/:market -> JSON from Redis cache
#[derive(Deserialize)]
struct MarketPath{ market:String }
#[derive(Serialize)]
struct ListResp<'a>{ market:&'a str, count:usize, rows:Vec<instruments::InstrumentRow> }

async fn list_instruments(State(ctx):State<AppCtx>, Path(MarketPath{market}):Path<MarketPath>) -> Result<Json<ListResp<'static>>, axum::http::StatusCode> {
    let key = match market.as_str() {
        "spot" => std::env::var("REDIS_KEY_SPOT").unwrap_or_else(|_| "instruments:binance:spot".to_string()),
        "um"   => std::env::var("REDIS_KEY_UM").unwrap_or_else(|_| "instruments:binance:um".to_string()),
        "cm"   => std::env::var("REDIS_KEY_CM").unwrap_or_else(|_| "instruments:binance:cm".to_string()),
        _ => return Err(axum::http::StatusCode::BAD_REQUEST),
    };
    match instruments::cache_get(&ctx.cfg.redis_url, &key).await {
        Ok(Some(rows)) => Ok(Json(ListResp{ market: Box::leak(market.into_boxed_str()), count: rows.len(), rows })),
        Ok(None) => Err(axum::http::StatusCode::NO_CONTENT),
        Err(_e) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn build_router(ctx: AppCtx) -> Router {
    let static_service = ServeDir::new("static").append_index_html_on_directories(true);

    Router::new()
        // was: .route("/ws/:exchange/:symbol/:interval", get(ws_handler))
        .route("/ws/{exchange}/{symbol}/{interval}", get(ws_handler))
        // was: .route("/api/instruments/:market", get(list_instruments))
        .route("/api/instruments/{market}", get(list_instruments))
        .route("/api/instruments/_sync", post(sync_now))
        .nest_service("/", static_service)
        .with_state(ctx)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

// ====================== main ======================
#[tokio::main]
async fn main() -> Result<()> {
    // logging
    let sub = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .finish();
    tracing::subscriber::set_global_default(sub).unwrap();

    // load .env
    let _ = dotenv();

    // configs
    let server_cfg = ServerCfg::from_env();
    let instr_cfg  = instruments::InstrumentsCfg::from_env();
    let ctx = AppCtx { cfg: server_cfg.clone(), hub: Hub::new(), book: Book::new(), instr_cfg: instr_cfg.clone() };

    // spawn consumers for default streams
    for s in server_cfg.streams.clone() {
        let ctx2 = ctx.clone();
        tokio::spawn(async move {
            if let Err(e)=run_consumer(ctx2, s.clone()).await { error!("consumer failed: {e:?}"); }
        });
    }

    // spawn periodic instruments sync task
    tokio::spawn(instruments::spawn_periodic_sync(instr_cfg));

    // serve http
    let listener = TcpListener::bind(server_cfg.bind_addr).await?;
    info!("HTTP listening on {}", server_cfg.bind_addr);
    axum::serve(listener, build_router(ctx)).await?;
    Ok(())
}
