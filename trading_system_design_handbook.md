
# Production Trading System Design Handbook
## 4. Trading Algorithm Research and Classification

The right framing is not “which strategy wins most often?” The right framing is:

> Which strategies have positive expectancy after costs under specific market regimes and can be implemented with data I actually have?

A robust platform should support multiple strategy families and a regime-aware selection layer.

### 4.1 Summary classification table

| Strategy Family | Core Idea | Required Data | Best Regime | Implementation Difficulty | Latency Needs |
|---|---|---|---|---|---|
| Trend-following | Ride persistent directional moves | OHLCV, trend filters | directional, expanding markets | Low to Medium | Low |
| Mean reversion | Fade temporary dislocations | OHLCV, VWAP, volatility | range-bound, mean-reverting | Low to Medium | Low |
| Breakout | Trade acceptance beyond established range | OHLCV, volume, vol compression | transition from compression to expansion | Medium | Low to Medium |
| Momentum | Continue recent strength/weakness | OHLCV, returns, relative strength | trending and rotation markets | Medium | Low |
| Market microstructure | Exploit book/trade short-horizon edge | ticks, L2/L3, execution feedback | liquid venues, short horizons | High | High |
| Volatility expansion/contraction | Trade transitions in volatility state | OHLCV, ATR, squeeze metrics | pre/post compression states | Medium | Low |
| Volume profile / VWAP | Trade around value, acceptance, auction logic | intraday volume, price distribution | intraday mean reversion or trend days | Medium | Low to Medium |
| Funding/basis/OI/order-flow | Use derivatives positioning context | funding, OI, basis, order flow | perp/futures dislocations | Medium to High | Low to Medium |
| Multi-timeframe confirmation | Align higher timeframe context with lower trigger | multi-timeframe bars/features | broad applicability | Medium | Low |
| Regime detection | Switch or disable strategies by state | features across trend/vol/liquidity | all markets | Medium | Low |

---

### 4.2 Trend-following

#### Core logic
Trend-following assumes price persistence. The system enters in the direction of an identified trend and exits when that trend weakens or invalidates.

#### Typical inputs
- OHLCV
- EMA/SMA slopes and spreads
- ADX / trend strength
- Donchian channel breakout levels
- session VWAP
- optional OI/funding confirmation for derivatives

#### Typical entry conditions
Examples:
- `EMA20 > EMA50 > EMA200`
- `close > EMA20`
- `ADX > 20`
- pullback into EMA20 or session VWAP followed by rejection candle
- breakout above N-bar high with acceptable spread and volume confirmation

#### Typical exit conditions
- close below trailing EMA
- break of prior swing low/high
- opposite signal
- time stop if no follow-through
- volatility spike against position

#### Suitable SL/TP patterns
- initial stop based on ATR or swing low/high
- partial scale-out at 1.5R or 2R
- trailing stop using ATR or moving average
- no fixed TP if strategy wants to capture long tails

#### Suitable regime
- directional, expansionary markets
- assets with persistent trend behavior
- sectors under strong narrative or macro flow

#### Advantages
- simple to code
- robust conceptually
- can catch outsized moves
- works on multiple timeframes

#### Weaknesses
- chopped up in range markets
- sensitive to delayed entries
- can give back open profits without strong exit logic

#### Easy-to-make mistakes
- trading trend signals during low-ADX chop
- entering too extended from moving average
- no market regime filter
- ignoring funding extremes in perp markets

#### Code implementation suitability
Very high. This should be one of the first strategies you implement.

#### Practical module split
- trend filter
- pullback detector
- breakout detector
- volatility guard
- trailing exit engine

---

### 4.3 Mean reversion

#### Core logic
Mean reversion assumes short-term dislocations revert toward fair value or equilibrium.

#### Typical fair-value anchors
- rolling mean
- EMA
- VWAP
- anchored VWAP
- value area high/low
- fair microprice for short horizons

#### Required inputs
- OHLCV
- volatility bands
- RSI / z-score
- VWAP distance
- regime filter
- optional order-flow slowdown or exhaustion clues

#### Typical entry conditions
Examples:
- z-score below `-2`
- RSI below 25 or 30
- close outside Bollinger lower band
- price far below intraday VWAP but no structural breakdown
- overshoot into support with absorption

#### Typical exit conditions
- mean reversion back to VWAP / EMA / value area
- signal normalization
- adverse continuation beyond stop
- time stop if bounce fails quickly

#### Suitable SL/TP patterns
- tight stop relative to expected snapback
- TP at mean or partial TP at half reversion + trail
- lower tolerance for delay than trend strategies

#### Suitable regime
- range, balanced auction, low directional persistence
- liquid assets with repeated overshoots

#### Advantages
- often better entry prices
- smaller stops possible
- high trade frequency possible

#### Weaknesses
- catastrophic if fading true trend continuation
- many “cheap gets cheaper” traps
- regime dependence is severe

#### Easy-to-make mistakes
- fading a breakout that is actually valid
- using mean-reversion logic during volatility expansion
- no event/news filter
- no trend strength rejection logic

#### Code implementation suitability
High. Good second baseline strategy after trend-following.

#### Practical module split
- equilibrium anchor module
- distance/z-score module
- oversold/overbought detector
- trend veto filter
- quick-exit module

---

### 4.4 Breakout

#### Core logic
Breakout strategies enter when price exits a well-defined range and shows evidence of acceptance beyond it.

#### Required inputs
- OHLCV
- local highs/lows or Donchian channels
- volatility compression metrics
- volume expansion
- spread and slippage conditions
- optional book pressure confirmation

#### Typical entry conditions
Examples:
- 20-bar range break
- Bollinger band squeeze or low ATR percentile before break
- close near candle high after break
- relative volume > threshold
- breakout bar not too extended relative to ATR

#### Typical exit conditions
- failed acceptance back inside range
- close back below breakout level
- opposite impulse
- trailing stop after range expansion

#### Suitable SL/TP patterns
- stop just inside broken range or ATR-based
- first target at range height projection
- trail once expansion confirms

#### Suitable regime
- compression to expansion transitions
- post-news directional continuation
- market open/session hand-off periods

#### Advantages
- can capture explosive moves
- clean rule definitions
- scalable from bar-based to microstructure-confirmed logic

#### Weaknesses
- fake breakouts frequent
- slippage can erase edge
- overtrading around obvious levels is common

#### Easy-to-make mistakes
- entering late after breakout candle exhaustion
- treating every level touch as breakout
- no retest/acceptance logic
- no breakout quality filter

#### Code implementation suitability
High, especially when paired with a quality filter and fake-break detector.

#### Practical module split
- level detector
- compression detector
- breakout validator
- quality score
- failure exit

---

### 4.5 Momentum

Momentum can mean:
1. **time-series momentum**: an asset continues moving in its recent direction
2. **cross-sectional momentum**: relatively stronger assets outperform weaker ones

#### Required inputs
- rolling returns
- relative strength rankings
- trend persistence metrics
- sector or correlated asset context
- volume and volatility filters

#### Typical entry conditions
- top-N relative strength universe selection
- positive return over medium lookback and positive short-term continuation
- price above moving average cluster
- momentum plus trend filter alignment

#### Typical exit conditions
- rank deterioration
- momentum roll-over
- volatility shock
- trailing stop or time-based rebalance

#### Best regime
- rotational markets
- trending markets with persistent leadership

#### Advantages
- can be turned into systematic ranking engine
- scalable across many symbols
- works well with portfolio construction

#### Weaknesses
- crashes during sudden reversals
- requires good universe filtering
- ranking noise if data quality is poor

#### Code implementation suitability
High for bar-based systems, medium for cross-sectional portfolio engines.

---

### 4.6 Market microstructure strategies

#### Core logic
Exploit short-horizon predictive signals in order flow, spread dynamics, queue imbalance, trade aggressor flow, and book resilience.

#### Required inputs
- tick data
- best bid/ask stream
- L2 or L3 order book
- aggressor trades
- cancellations, replenishment, spread
- latency measurements and fill model

#### Example signals
- top-of-book imbalance
- microprice deviation from mid
- aggressive buy burst with book thinning on ask
- repeated absorption at a level
- spread collapse with directional trade imbalance
- queue depletion leading short-term move

#### Typical entry conditions
- imbalance threshold crossed
- trade flow confirms
- spread acceptable
- queue risk acceptable
- no venue degradation

#### Typical exit conditions
- edge decay within seconds
- book flips
- spread widens
- fill risk exceeds expected edge
- inventory/risk cap

#### SL/TP patterns
- usually small and very tight
- often inventory/edge/time based rather than chart based

#### Best regime
- highly liquid instruments
- stable low-latency feed/execution
- venues where queue priority matters

#### Advantages
- potentially strong short-horizon alpha
- independent from medium-term chart patterns
- excellent for execution-aware systems

#### Weaknesses
- data and infrastructure intensive
- edge decays quickly
- backtest realism is hard
- queue modeling is non-trivial

#### Easy-to-make mistakes
- using candle data to simulate microstructure alpha
- ignoring latency and matching-engine behavior
- no partial fill model
- no realistic fees/slippage

#### Code implementation suitability
Medium to High difficulty. Do not start here unless you already have tick/L2/L3 infrastructure.

---

### 4.7 Volatility expansion / contraction

#### Core logic
Volatility is cyclical. Compression often precedes expansion; expansion often mean-reverts or transitions into trend.

#### Required inputs
- ATR and ATR percentile
- realized volatility
- Bollinger bandwidth
- true range expansion
- volume context
- session timing

#### Example entry styles
- enter on breakout after low-volatility squeeze
- fade late expansion after exhaustion if trend quality is weak
- switch stop width and sizing based on volatility state

#### Best regime
- transition zones
- session opens
- pre/post catalyst periods

#### Advantages
- useful both as a standalone strategy and as a strategy filter
- improves stop placement and position sizing

#### Weaknesses
- volatility increase does not guarantee direction
- needs directional confirmation to avoid noise

#### Code implementation suitability
Very high. Even if not standalone, use it as a filter.

---

### 4.8 Volume profile / VWAP-based logic

#### Core logic
Auction-market style interpretation:
- price around fair value rotates
- price away from fair value either rejects and reverts, or accepts and trends

#### Required inputs
- intraday trades/volume
- session VWAP
- anchored VWAP
- volume-at-price histogram / profile
- value area high/low, HVN/LVN
- session context

#### Typical entry conditions
- VWAP reclaim in trend direction
- rejection from value area boundary
- LVN breakout into price discovery
- pullback to anchored VWAP in directional session

#### Exits
- target opposing value area edge
- target VWAP reversion
- trail during acceptance outside value

#### Best regime
- intraday auction markets
- instruments with reliable volume distributions

#### Advantages
- strong market-structure intuition
- great combination with breakout and mean reversion
- more context-rich than plain RSI signals

#### Weaknesses
- requires intraday trade volume quality
- session boundaries and anchor choice matter
- easier to misuse on illiquid markets

#### Code implementation suitability
Medium. Straightforward once trade aggregation pipeline exists.

---

### 4.9 Funding / basis / open-interest / order-flow driven strategies

#### Core logic
In derivatives markets, positioning and carry matter. Price alone is not enough.

#### Required inputs
- funding rate
- predicted funding rate
- open interest
- long-short ratio if available
- spot/perp basis
- liquidation flow if available
- trade/order flow

#### Example patterns
- price up + OI up + moderate funding = continuation-friendly
- price up + OI up + funding extremely positive = squeeze/exhaustion risk
- price down + OI down = long liquidation unwinding
- perp premium dislocation reverting toward spot
- basis mean reversion or cash-and-carry style logic

#### Exits
- normalization of basis/funding
- OI regime change
- adverse price move beyond invalidation
- funding window risk passed

#### Best regime
- active perp/futures markets
- high retail leverage environments
- event-driven liquidations

#### Advantages
- adds non-price context
- useful as filter for chart-based strategies
- helps avoid crowded continuation entries

#### Weaknesses
- data availability varies by venue
- cross-venue alignment can be messy
- retail long-short ratio can be noisy

#### Code implementation suitability
Medium to High depending on data collection maturity.

---

### 4.10 Multi-timeframe confirmation

#### Core logic
Use higher timeframe for context, lower timeframe for trigger.

A common pattern:
- 4H trend says “only long”
- 15m says “pullback complete”
- 1m says “enter on reclaim / order flow confirm”

#### Required inputs
- multi-timeframe OHLCV/features
- consistent timestamp alignment
- strict point-in-time feature joins

#### Advantages
- reduces false signals
- prevents trading lower timeframe noise against higher timeframe trend

#### Weaknesses
- can delay entry
- feature alignment bugs are common
- too many filters can kill trade frequency

#### Code implementation suitability
High.

---

### 4.11 Regime detection

#### Core logic
Do not ask one strategy to solve every market condition. First classify the market state, then route to:
- trend strategy
- mean reversion strategy
- breakout strategy
- no-trade state

#### Example regime labels
- `TREND_UP`
- `TREND_DOWN`
- `RANGE`
- `VOL_COMPRESSION`
- `VOL_EXPANSION`
- `PANIC`
- `ILLIQUID`
- `HIGH_SPREAD_NO_TRADE`

#### Inputs
- trend strength
- volatility percentile
- spread/liquidity
- volume participation
- macro session/event context
- optional cross-asset leadership

#### Implementation styles
- threshold state machine
- hidden Markov model
- classifier
- scoring system

#### Why this matters
A strategy framework with regime detection is usually more robust than a single monolithic strategy with many exceptions.

---

## 5. Feature Engineering for Better Signal Quality

The purpose of features is not to create as many numbers as possible. The purpose is to represent market state in a way that improves decision quality, ranking quality, or risk filtering.

### 5.1 Feature engineering principles

1. **Every feature must have a trading meaning**
2. **Every feature must be point-in-time correct**
3. **Every feature must declare its lookback and update cadence**
4. **Prefer incremental computation over full-window recomputation in live engines**
5. **Use the same formula online and offline**
6. **Monitor missingness, staleness, and drift**
7. **Features should help either direction, timing, quality, or risk**

---

### 5.2 Price action features

| Feature | Formula / Logic | Trading Meaning | Good Timeframes | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `log_return_n` | `ln(close_t / close_t-n)` | directional impulse | all | momentum/trend ranking | vol filter | noisy on tiny windows |
| `range_n` | `high_n - low_n` or rolling range | expansion/compression | all | breakout setup | volume ratio | insensitive to direction |
| `candle_body_ratio` | `abs(close-open)/(high-low+eps)` | conviction inside candle | 1m to 4H | breakout/trend continuation | volume spike | single candles can mislead |
| `upper_wick_ratio` | `(high-max(open,close))/(range+eps)` | rejection from highs | all | trend exhaustion / fake breakout | RSI, OBI | depends on data resolution |
| `lower_wick_ratio` | `(min(open,close)-low)/(range+eps)` | rejection from lows | all | sweep/reversal detection | delta proxy | same caveat |
| `close_location_value` | `(close-low)/(high-low+eps)` | where close sits in range | intraday+ | follow-through quality | breakout logic | zero-range candles |
| `distance_from_high_n` | `(rolling_high_n-close)/close` | extension below resistance | 5m+ | breakout readiness | compression features | regime-dependent |
| `distance_from_low_n` | `(close-rolling_low_n)/close` | extension above support | 5m+ | pullback quality | trend filter | regime-dependent |

**Implementation note:** Price action features are best when paired with regime and volatility context. A strong bullish candle in a compression regime means something different than the same candle after a 4-sigma move.

---

### 5.3 Trend features

| Feature | Formula / Logic | Trading Meaning | Good Timeframes | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `ema_dist_k` | `(close-EMA_k)/EMA_k` | extension relative to trend anchor | all | pullback/trend-following | ATR%, RSI | can be too lagging alone |
| `ema_spread_fast_slow` | `(EMA_fast-EMA_slow)/EMA_slow` | trend structure strength | 5m+ | trend filter | ADX, volume | delayed on reversals |
| `ema_slope_k` | `EMA_k - EMA_k_prev` | directional slope | all | regime classification | spread/liquidity | slope unit needs normalization |
| `adx_n` | standard ADX | directional persistence strength | 15m+ | trend vs range filter | moving averages | late in some markets |
| `regression_slope_n` | OLS slope over lookback | trend line strength | 15m+ | momentum scoring | R² | sensitive to outliers |
| `hh_hl_score` | structural higher-high/higher-low logic | market structure confirmation | 5m+ | trend and pullback continuation | swing points | requires clean pivot logic |

**Use case:** Trend features are essential not only for trend strategies, but also for vetoing mean-reversion trades.

---

### 5.4 Volatility features

| Feature | Formula / Logic | Trading Meaning | Good Timeframes | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `atr_n` | Average True Range | stop width, noise floor | all | stop sizing | position sizing | price-scale dependent |
| `atr_pct_n` | `ATR_n / close` | normalized volatility | all | cross-asset comparison | leverage rules | spikes around gaps |
| `realized_vol_n` | std of log returns | actual recent volatility | 1m+ | regime state | breakout filters | scaling choices matter |
| `vol_percentile_n` | percentile of vol in rolling history | whether vol is extreme | 5m+ | compression/expansion classifier | session features | unstable under regime shift |
| `bb_width_n` | `(upper-lower)/middle` | squeeze measure | 5m+ | breakout prep | volume ratio | duplicate of vol features if overused |
| `parkinson_vol_n` | range-based estimator | intrabar volatility estimate | 5m+ | richer vol estimate | ATR | less useful with noisy highs/lows |

**Trading meaning:** Volatility features determine whether you should even trade, what stop width to use, and how much size is justified.

---

### 5.5 Volume features

| Feature | Formula / Logic | Trading Meaning | Good Timeframes | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `volume_ratio_n` | `volume / SMA(volume,n)` | participation surge | all | breakout confirmation | candle body ratio | distorted by session seasonality |
| `relative_volume_tod` | volume vs same time-of-day baseline | unusual activity for that clock time | intraday | session-aware trading | time-of-day features | requires seasonal baseline |
| `obv_n` | On-Balance Volume | cumulative pressure proxy | 15m+ | trend confirmation | price slope | noisy intraday |
| `buy_sell_volume_ratio` | `buy_volume / sell_volume` | directional pressure | 1m+ | intraday continuation | OBI | depends on aggressor classification quality |
| `volume_at_price_density` | volume histogram concentration | value acceptance | intraday | profile logic | VWAP/profile | requires trade-level aggregation |

---

### 5.6 Momentum features

| Feature | Formula / Logic | Trading Meaning | Good Timeframes | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `roc_n` | `(close/close_n)-1` | raw momentum | all | ranking, continuation | vol filter | noisy |
| `rsi_n` | standard RSI | exhaustion/impulse gauge | 5m+ | reversion or continuation depending regime | trend filter | regime-sensitive |
| `stoch_rsi` | RSI normalized within range | fast momentum oscillator | 1m+ | short-term timing | trend direction | whipsaws in chop |
| `macd_hist` | MACD histogram | acceleration/deceleration | 15m+ | trend strength and roll-over | EMA structure | lagging in fast markets |
| `momentum_persistence` | fraction of up bars in lookback | directional consistency | all | trend quality | range compression | simplistic alone |

---

### 5.7 Order book imbalance and microstructure features

These features require tick/L2/L3 data and much stricter implementation discipline.

| Feature | Formula / Logic | Trading Meaning | Useful Horizon | Combine With | Caveats |
|---|---|---|---|---|---|
| `obi_top_k` | `(sum(bid_sz_1..k)-sum(ask_sz_1..k))/total` | near-book pressure | milliseconds to seconds | trade aggressor flow | spoofing can distort |
| `microprice` | weighted mid using top bid/ask sizes | short-horizon fair value | sub-second to seconds | spread, trade burst | only useful in liquid books |
| `spread_ticks` | `(ask-bid)/tick_size` | transaction cost and fragility | all short horizons | urgency and execution | hard veto in wide spread |
| `queue_imbalance` | queue size imbalance at best levels | fill probability and direction | sub-second | passive execution logic | venue-specific semantics |
| `cancel_rate` | order cancellations per unit time | unstable liquidity/spoofing clues | seconds | OBI, spread | may be exchange-noise heavy |
| `book_replenishment_ratio` | replenishment after aggressive hits | absorption vs depletion | seconds | sweep detection | requires book event tracking |

**Critical warning:** book imbalance features can look predictive in historical data and fail live due to latency, hidden orders, queue priority, or stale-book effects.

---

### 5.8 Liquidity sweep, absorption, and spoofing-like clue features

These are not perfect truth labels. They are heuristics.

| Feature / Pattern | Logic | Trading Meaning | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|
| `liquidity_sweep_flag` | fast move through local high/low + aggressive volume burst + wick | stops likely triggered | breakout fade or continuation analysis | wick ratio, delta proxy | many false positives |
| `absorption_score` | repeated aggressive prints into a level with little price progress | passive side absorbing flow | reversal or breakout failure analysis | OBI, replenishment | requires trade+book alignment |
| `spoofing_like_score` | large visible size appears then disappears before trade | possibly deceptive liquidity | execution/risk filter | cancel rate, spread | do not over-interpret as intent |
| `iceberg_hint_score` | repeated fills at same level with resilient displayed size | hidden liquidity suspicion | breakout filter | replenishment ratio | inference only, not ground truth |

**Implementation note:** treat these as soft features, never as hard proof of manipulation.

---

### 5.9 Funding, open interest, long-short ratio, and basis features

| Feature | Formula / Logic | Trading Meaning | Timeframe | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `funding_rate_raw` | current/next funding rate | crowd carry pressure | 1h to 8h | perp context | trend, OI delta | venue-specific regimes |
| `funding_zscore` | standardized funding over rolling lookback | extreme positioning | 4h+ | squeeze risk filter | basis, price trend | non-stationary |
| `oi_delta_n` | change in open interest | build-up or flush of positioning | 5m+ | continuation vs liquidation interpretation | price return | venue aggregation issues |
| `price_oi_divergence` | compare price move and OI change | separates short covering from fresh positioning | 5m+ | derivatives context | funding | interpretation still heuristic |
| `long_short_ratio` | retail or account positioning ratio | crowding indicator | 1h+ | contrarian filters | funding, basis | often noisy |
| `basis_pct` | `(deriv_mid-spot_mid)/spot_mid` | premium/discount | minutes to daily | arbitrage/dislocation context | funding, OI | requires synced spot/deriv data |
| `basis_annualized` | normalized basis over time to expiry or funding interval | carry attractiveness | hourly+ | basis strategies | term structure | formula differs by product |

---

### 5.10 Time-of-day and session features

| Feature | Logic | Why It Matters | Use Cases | Caveats |
|---|---|---|---|---|
| `hour_sin`, `hour_cos` | cyclical encoding of time | avoids discontinuity at hour boundaries | models/classifiers | timezone correctness |
| `session_tag` | Asia / Europe / US / overlap | market behavior changes by session | strategy gating | crypto is 24/7 but still session-structured |
| `minutes_since_session_open` | clock distance from open | opening volatility effects | breakout/vol filters | define session consistently |
| `funding_window_proximity` | minutes to next funding | order flow distortion around funding | no-trade windows / filters | venue-specific funding times |
| `day_of_week` | categorical or cyclical | recurring seasonality | filters and analytics | weak standalone signal |

---

### 5.11 Cross-asset and market-context features

| Feature | Logic | Trading Meaning | Use Cases | Caveats |
|---|---|---|---|---|
| `btc_return_lead_n` | BTC recent return | altcoins often follow BTC | alt filters | may lag or decouple |
| `rolling_beta_to_btc` | beta estimate vs BTC | position sizing and hedging | portfolio risk | unstable in crises |
| `sector_strength_score` | average return of asset cluster | theme rotation | ranking | universe definition matters |
| `dominance_change` | BTC dominance / major index move | capital rotation context | alt strategies | external data needed |
| `correlation_regime` | rolling correlation matrix state | diversification and contagion risk | portfolio construction | fragile with short samples |

---

### 5.12 Regime classification features

| Feature | Logic | Purpose | Best Use |
|---|---|---|---|
| `trend_strength_score` | blend of EMA spread, slope, ADX | identify trend state | strategy selection |
| `vol_state_score` | ATR percentile + realized vol percentile | identify compression/expansion/panic | risk sizing and strategy routing |
| `liquidity_score` | spread, depth, trade rate | identify tradability | no-trade filters |
| `participation_score` | relative volume, trade count | detect active participation | breakout validation |
| `stability_score` | gap frequency, feed health, stale-book checks | operational state | execution safeguard |

---

### 5.13 Feature combinations that are especially useful

#### Combination A: Trend continuation quality
- `ema_spread_fast_slow`
- `ema_slope`
- `pullback_distance_to_ema`
- `volume_ratio`
- `atr_pct`
- `funding_zscore` as crowding filter

#### Combination B: Breakout quality
- `range_compression_score`
- `breakout_close_location`
- `volume_ratio`
- `relative_spread`
- `obi_top_k`
- `retest_acceptance_flag`

#### Combination C: Mean reversion quality
- `zscore_from_vwap`
- `rsi`
- `trend_strength_score` as veto
- `absorption_score`
- `vol_state_score`

#### Combination D: Liquidation / squeeze context
- `price_return`
- `oi_delta`
- `funding_zscore`
- `basis_pct`
- `trade_flow_imbalance`

---

### 5.14 Feature store design rules for real systems

- version every feature definition
- persist feature-generation metadata
- include `feature_time` and `available_time` if asynchronous sources exist
- use a shared feature library between research and live
- validate null rate and stale rate
- never compute training features using future-corrected or backfilled values if live would not have them
- store online feature snapshots used to make each live decision

---

## 6. Codifying Professional Trading Rules into a Rule Engine

This section translates common professional trading principles into explicit, machine-implementable rules. The idea is not to imitate specific personalities; the idea is to codify recurring rules that consistently appear in professional discretionary and systematic practice.

### 6.1 Rule engine design goals

A rule engine should answer four questions:

1. **Am I allowed to trade right now?**
2. **Is this setup valid?**
3. **How large can I trade?**
4. **When must I reduce, exit, or disable trading?**

### 6.2 Rule categories

- market context rules
- signal confirmation rules
- no-trade rules
- position sizing rules
- order placement rules
- add/reduce rules
- stop and take-profit rules
- drawdown and session loss rules
- portfolio correlation rules
- operational safety rules

---

### 6.3 Rule table: professional principle → code-ready rule

| Principle | Rule Logic | Example Condition | Action |
|---|---|---|---|
| Trade with context, not isolated candles | only trade if setup aligns with regime | `regime in {TREND_UP, VOL_EXPANSION_UP}` for longs | reject otherwise |
| Preserve capital first | no new risk after drawdown threshold | `daily_loss_r <= -3` | disable new entries for day |
| Do not trade noise | reject when spread/vol/liquidity poor | `spread_ticks > max_spread_ticks` or `liquidity_score < min` | reject |
| Require confirmation | entry only if trigger + context + risk pass | `setup.pass && confirm.pass && risk.pass` | allow |
| Never widen stop | stop may tighten, not loosen | `new_stop <= old_stop` for longs only if tighter | reject wider stop |
| Add only to winners | pyramiding only after favorable move and improved stop | `unrealized_r >= 1 && regime_supportive` | allow add |
| Cut losers quickly | immediate exit when invalidation occurs | `mark_price <= stop_price` or `breakout_failed` | exit |
| Take asymmetric trades | require minimum reward/risk or edge | `expected_rr >= min_rr` | reject low-R trades |
| Avoid revenge trading | cap trade count after losses | `consecutive_losses >= max` | cool-down |
| Protect weekly capital | aggregate weekly risk shutdown | `weekly_loss_r <= -8` | disable entries |
| Avoid correlated overexposure | sum risk across cluster | `cluster_gross_exposure > limit` | reject new correlated trade |
| Do not trade stale data | market feed freshness check | `now - last_market_event_ms > threshold` | reject/cancel |
| No trading during degraded exchange state | venue health required | `venue_health != OK` | reject |
| Limit churn | minimum spacing between entries | `now - last_entry_time < cooldown_ms` | reject |

---

### 6.4 Concrete code-oriented rule logic

#### Context rule
```text
IF regime not in allowed_regimes(strategy, side)
THEN reject signal
```

#### Signal confirmation rule
```text
IF setup_score < min_setup_score
OR confirmation_score < min_confirmation_score
THEN reject signal
```

#### Risk-reward rule
```text
expected_reward = target_price - entry_price
risk = entry_price - stop_price
IF expected_reward / risk < min_rr
THEN reject signal
```

#### Daily loss shutdown
```text
IF realized_pnl_today + unrealized_pnl_open_positions <= -daily_loss_limit
THEN disable new entries until next session
```

#### Max exposure rule
```text
IF current_gross_exposure + proposed_exposure > max_gross_exposure
THEN reject signal
```

#### Correlation cluster rule
```text
IF proposed_trade.cluster == existing_position.cluster
AND cluster_risk + proposed_risk > cluster_risk_limit
THEN reject signal
```

#### Spread sanity rule
```text
IF spread_ticks > max_spread_ticks_for_strategy
THEN reject signal
```

#### Volatility sanity rule
```text
IF atr_pct > max_atr_pct
OR vol_state == PANIC
THEN reject signal
```

#### Trade frequency rule
```text
IF trades_today(strategy, instrument) >= max_trades_per_day
THEN reject signal
```

#### No-stale-data rule
```text
IF current_time - last_feature_update_time > feature_stale_threshold_ms
THEN reject signal
```

---

### 6.5 Rules for adding, reducing, and exiting positions

#### Add-to-winner rule
Allow adds only if all are true:
- current position is profitable
- regime still valid
- stop can be tightened or held without increasing total portfolio risk beyond limit
- new add is not chasing parabolic extension
- cumulative risk remains within allowed cap

#### Reduce-position rule
Reduce when:
- target 1 hit
- volatility spikes against position
- order flow reverses
- basis/funding context deteriorates
- higher timeframe invalidation appears
- portfolio needs de-risking

#### Stop-loss rule
A stop should be derived from one of:
- structure invalidation
- ATR multiple
- volatility-adjusted value area failure
- microstructure edge decay
- time-based invalidation

Bad stop logic:
- arbitrary wide stop to “avoid being wicked”
- stop widened after entry without new thesis

#### Take-profit rule
Common valid implementations:
- fixed R multiple
- opposing structure
- reversion to fair value
- scale-out plus trailing remainder
- dynamic exit when score decays below threshold

---

### 6.6 Conditions where entry must be forbidden

A production engine should explicitly define `NO_TRADE` conditions. Examples:

- spread too wide
- book too thin
- stale data
- venue reconnecting / sequence gap unresolved
- within maintenance or funding window
- daily or weekly risk shutdown
- strategy health degraded
- too many open positions
- low-quality breakout score
- mean-reversion attempt against strong trend regime
- event/news blackout if strategy is not event-driven
- feature missingness exceeds threshold

---

### 6.7 Example rule-engine evaluation order

```text
1. Operational health checks
2. Market data freshness checks
3. Venue tradability checks
4. Regime checks
5. Strategy-specific setup checks
6. Signal confirmation checks
7. Risk-reward checks
8. Position sizing checks
9. Portfolio / correlation checks
10. Final order intent creation
```

This order reduces wasted computation and keeps the reasoning trace clean.

---

## 7. Algorithms You Can Build Yourself

This section focuses on modular trading algorithms that are worth building from scratch because they create reusable infrastructure and do not lock you into one brittle strategy.

### 7.1 Signal scoring engine

#### Core idea
Instead of binary indicator logic, compute a weighted score representing trade quality.

#### Inputs
- trend features
- volatility features
- volume features
- regime label
- execution cost estimates

#### Output
- `score`
- `confidence`
- `side`
- `reason_codes`

#### General logic
```text
score = w1*trend + w2*momentum + w3*breakout_quality
      + w4*volume_confirmation + w5*regime_alignment
      - w6*execution_cost_penalty - w7*crowding_penalty
```

#### Module split
- feature normalization
- factor scoring
- weighting
- confidence calibration
- thresholding
- explanation generator

#### Difficulty
Low to Medium

#### Mandatory data
OHLCV minimum, better with funding/OI and spread metrics

#### Best simple first version
Start with 5-8 hand-engineered components and fixed weights. Do not start with ML weighting.

---

### 7.2 Weighted multi-factor model

#### Core idea
Combine multiple orthogonal factors:
- trend
- mean reversion distance
- volatility state
- volume participation
- market context

#### Inputs
- feature vector at decision time
- per-factor weights
- optional side-specific rules

#### Output
- long score
- short score
- hold score

#### General logic
- compute each factor on normalized scale, e.g. `[-1, 1]` or `[0, 1]`
- aggregate by weighted sum
- pass through gating rules
- map to action and size bucket

#### Module split
- factor calculators
- factor registry
- score composer
- side resolver
- threshold config

#### Difficulty
Medium

#### Mandatory data
OHLCV plus context features

#### Best simple first version
Use 4 factors and static weights, with regime gating.

---

### 7.3 Regime-aware strategy selector

#### Core idea
First classify market regime, then route traffic to the strategy best suited for that regime.

#### Inputs
- trend strength
- volatility state
- spread/liquidity
- volume participation
- optional derivatives context

#### Output
- `selected_strategy_family`
- `allowed_sides`
- `risk_mode`

#### General logic
```text
if illiquid or stale -> NO_TRADE
elif trend_strength high and vol stable -> TREND
elif low trend and balanced auction -> MEAN_REVERSION
elif compression then breakout conditions -> BREAKOUT
elif panic -> REDUCED_RISK or NO_TRADE
```

#### Module split
- regime feature builder
- regime classifier
- strategy router
- fallback/no-trade policy

#### Difficulty
Medium

#### Mandatory data
OHLCV + spread/liquidity + vol state

#### Best simple first version
Use a hand-tuned state machine before ML classification.

---

### 7.4 Order flow confirmation engine

#### Core idea
Use short-horizon trade/book behavior to confirm or reject a higher-level setup.

#### Inputs
- aggressor trade flow
- book imbalance
- spread
- replenishment / depletion
- microprice trend

#### Output
- `pass: boolean`
- `score`
- `reason`

#### Example uses
- only take breakout if order flow confirms
- only fade sweep if absorption is detected
- veto mean reversion if aggressive continuation persists

#### Module split
- order flow window aggregator
- imbalance calculator
- sweep/absorption detector
- execution cost estimator

#### Difficulty
High

#### Mandatory data
Ticks + L2 at minimum

#### Best simple first version
Use only:
- top-5 imbalance
- aggressor flow delta
- spread filter

---

### 7.5 Breakout quality filter

#### Core idea
Not all breakouts are equal. Score breakout quality before allowing entry.

#### Inputs
- range compression score
- breakout candle structure
- close location
- relative volume
- spread
- distance from level
- retest acceptance

#### Output
- breakout quality score
- allow/reject

#### Logic
High-quality breakout characteristics:
- break after compression
- strong close near extreme
- above-normal participation
- acceptable spread/slippage
- not already too extended from stop anchor

#### Module split
- level detector
- compression scorer
- participation scorer
- structural validator
- late-entry penalty

#### Difficulty
Medium

#### Mandatory data
OHLCV minimum, preferably volume and tick spread

#### Best simple first version
Score 5 components equally and require a threshold.

---

### 7.6 Fake breakout detector

#### Core idea
Detect breakouts likely to fail quickly.

#### Inputs
- breakout failure back into range
- long upper/lower wick
- weak close
- low relative volume
- adverse order flow after break
- mean reversion regime

#### Output
- `fake_breakout_probability` or rule-based flag

#### Module split
- failed acceptance detector
- wick/rejection detector
- volume insufficiency detector
- reversal-flow detector

#### Difficulty
Medium to High

#### Mandatory data
OHLCV and, for better quality, order flow

#### Best simple first version
Use:
- close back inside range within N bars
- low volume
- wick ratio
- ADX low or range regime

---

### 7.7 Trend exhaustion detector

#### Core idea
Identify when a trend is still up/down in price but losing quality.

#### Inputs
- momentum deceleration
- volatility spike
- distance from EMA/VWAP
- divergence (price vs RSI/MACD/OI)
- rejection candles
- funding extremes

#### Output
- `exhaustion_score`
- reduce/exit/veto-new-entry recommendation

#### Module split
- extension calculator
- momentum-decay calculator
- divergence detector
- crowding detector
- exit advisor

#### Difficulty
Medium

#### Mandatory data
OHLCV; funding/OI improve quality

#### Best simple first version
Use:
- EMA distance z-score
- RSI divergence or MACD roll-over
- wick rejection
- funding extreme filter

---

### 7.8 Volatility state classifier

#### Core idea
Classify market into states such as:
- compressed
- normal
- expanding
- panic

#### Inputs
- ATR percentile
- realized vol percentile
- range expansion
- spread regime
- gap / jump frequency

#### Output
- `vol_state`
- `recommended_size_multiplier`
- `strategy_permissions`

#### Difficulty
Low to Medium

#### Mandatory data
OHLCV and spread if intraday

#### Best simple first version
Threshold classifier with 4 states.

---

### 7.9 Entry confidence ranking

#### Core idea
If many signals fire at once, rank them and only take the best.

#### Inputs
- signal score
- expected RR
- spread/slippage penalty
- regime alignment
- cross-asset crowding / correlation

#### Output
- ranked candidate list
- allocate top K only

#### Why useful
This improves capital efficiency and reduces low-quality marginal trades.

#### Difficulty
Low

#### Mandatory data
Whatever feeds the signal engine plus cost estimates

#### Best simple first version
Weighted rank of `signal_score`, `RR`, and `cost_penalty`.

---

### 7.10 Adaptive TP/SL engine

#### Core idea
Target and stop distances should respond to regime and signal quality, not be fixed constants.

#### Inputs
- ATR%
- regime
- structure levels
- liquidity/spread
- confidence score

#### Output
- stop price
- target ladder
- trailing rules
- time stop

#### Example logic
- trend regime: wider stop, looser trail, larger target
- mean reversion: tighter stop, target at equilibrium
- panic regime: smaller size, wider stop only if expected edge justifies

#### Module split
- structure stop engine
- ATR stop engine
- target engine
- trailing engine
- time decay engine

#### Difficulty
Medium

#### Mandatory data
OHLCV at minimum

#### Best simple first version
Choose between:
- ATR stop
- structure stop
- VWAP target
based on regime label.

---

### 7.11 Position sizing engine by volatility and confidence

#### Core idea
Size should depend on both risk per trade and expected quality.

#### Inputs
- account equity
- max risk budget
- stop distance
- instrument volatility
- signal confidence
- correlation exposure
- leverage constraints

#### Output
- target size
- leverage
- allowed notional
- reject/approve flag

#### Example logic
```text
base_risk = equity * risk_fraction
raw_size = base_risk / stop_distance
vol_adjusted_size = raw_size * vol_multiplier
confidence_adjusted_size = vol_adjusted_size * confidence_multiplier
final_size = min(vol_adjusted_size, portfolio_limits, venue_limits)
```

#### Difficulty
Low to Medium

#### Mandatory data
equity, stop distance, volatility, venue rules

#### Best simple first version
Volatility-normalized fixed-fractional sizing.

---

### 7.12 Strategy ensemble

#### Core idea
Run several strategy families and combine them under allocation and conflict rules.

#### Inputs
- scores from multiple strategies
- regime label
- portfolio exposure
- capital constraints

#### Output
- final side
- final size
- selected strategies or weighted blend

#### Example policies
- regime selects one primary strategy family
- ensemble takes consensus of strategies
- portfolio layer allocates weights by recent stability
- conflicting signals cancel or reduce

#### Difficulty
Medium to High

#### Mandatory data
same as all child strategies

#### Best simple first version
Two-strategy ensemble:
- trend-following
- mean reversion
with regime routing and common risk engine.

---

## 8. Implementation Architecture and Code Modules

### 8.1 Language split recommendation

Choose language by latency sensitivity and system role.

| Layer | Good Language Choices | Notes |
|---|---|---|
| Research, feature prototyping, model training | Python | Fastest iteration, rich numeric stack |
| Control plane, APIs, dashboards, orchestration | Node.js / TypeScript, Go | Strong productivity and service tooling |
| Hot path execution, book processing, microstructure | Rust / C++ / Java | Better for tight latency budgets |
| SQL/OLAP transforms | SQL + Python | Use db-native materialization where possible |

**Practical recommendation:**  
If you are not yet doing true sub-millisecond microstructure trading, a Python research stack plus TypeScript/Node.js service layer is perfectly workable.  
If you move into L2/L3 short-horizon execution, move the hot path into Rust/C++/Java and keep Node.js as control plane.

### 8.2 Recommended service boundaries

```text
apps/
  market-ingest/
  market-normalizer/
  feature-engine/
  signal-engine/
  risk-engine/
  execution-engine/
  reconciliation-engine/
  replay-engine/
  analytics-api/
  config-service/

packages/
  domain-models/
  db-access/
  indicators/
  feature-library/
  strategy-sdk/
  risk-sdk/
  execution-sdk/
  event-contracts/
  common-utils/
```

### 8.3 Domain objects you should formalize

At minimum:

- `MarketContext`
- `FeatureSnapshot`
- `SignalDecision`
- `RiskDecision`
- `OrderIntent`
- `OrderState`
- `Fill`
- `PositionState`
- `StrategyLog`
- `ReplayContext`

### 8.4 Design your strategy interface for testability

Recommended idea:
- input is immutable context
- output is structured decision
- strategy should not send orders directly

#### Example decision contract
```ts
export interface DetectorResult {
  pass: boolean;
  reason: string;
  note?: string;
  score?: number;
}

export interface SignalDecision {
  strategyId: string;
  strategyVersion: string;
  instrumentId: string;
  eventTime: number;
  side: 'LONG' | 'SHORT' | 'HOLD' | 'EXIT' | 'REDUCE';
  score: number;
  confidence: number;
  actionable: boolean;
  reasonCodes: string[];
  stopPrice?: number;
  targetPrice?: number;
  metadata?: Record<string, unknown>;
}
```

### 8.5 Suggested market data processing pipeline

```text
raw exchange payload
  -> schema validate
  -> map symbols and venue fields
  -> assign event_time and ingest_time
  -> update latest in-memory state
  -> emit canonical event
  -> persist raw and canonical versions if needed
  -> trigger incremental feature updates
```

### 8.6 Incremental feature engine rules

To keep live latency under control:

- maintain rolling windows in memory
- update only features touched by new event
- avoid full-dataframe recomputation in live path
- precompute higher-timeframe bars incrementally
- snapshot live feature state periodically for recovery

### 8.7 Execution engine responsibilities

The execution engine is not “just place order”.

It must:
- translate `OrderIntent` into venue-specific order request
- generate deterministic `client_order_id`
- handle retries carefully
- track acknowledgements and rejects
- manage amend/cancel state machine
- reconcile fills and fees
- emit order events
- support kill switch behavior

### 8.8 Risk engine responsibilities

The risk engine should expose functions like:
- `checkSignalRisk`
- `checkOrderRisk`
- `checkPortfolioRisk`
- `checkOperationalRisk`
- `computePositionSize`
- `shouldDisableTrading`

It should be able to answer:
- whether a trade is allowed
- how large it may be
- whether current open positions must be reduced

### 8.9 Recommended processing sequence for a live decision

```text
1. Receive market event
2. Update market state
3. Update relevant features
4. Build MarketContext
5. Run strategy evaluation
6. Run risk evaluation
7. Build OrderIntent if approved
8. Send order via execution engine
9. Persist signal/risk/order events
10. Await order events/fills and update state
```

---

## 9. Backtest, Replay, Validation, and Monitoring

### 9.1 Backtest realism checklist

A backtest is not credible unless it models:

- exchange fees
- maker/taker differences
- slippage
- spread
- latency or delayed fill assumptions
- partial fills if needed
- funding rates for perps
- contract size/tick/lot precision
- rejected orders and no-fill scenarios where relevant

### 9.2 Replay engine requirements

Replay should be able to:
- consume the exact event order
- rebuild order book from snapshot + deltas
- reproduce feature calculations
- reproduce strategy decisions
- reproduce risk checks
- compare simulated and recorded live actions

### 9.3 Validation metrics beyond win rate

Track at least:

- expectancy
- average win / average loss
- payoff ratio
- profit factor
- Sharpe / Sortino if appropriate
- max drawdown
- time under water
- turnover
- fees and funding drag
- slippage bps
- fill ratio
- markout after fills
- performance by regime
- performance by session
- performance by feature bucket
- false breakout rate
- stop-out rate by setup type

### 9.4 Monitoring for live systems

Monitor:
- feed lag
- sequence gaps
- stale features
- strategy heartbeat
- order ack latency
- cancel latency
- reject rate
- fill rate
- PnL drift
- position reconciliation differences
- risk breach count
- database sink lag
- event-bus consumer lag

### 9.5 Research-live parity tests

Before shipping a strategy:
- run historical backtest
- run exact session replay
- run paper trading
- compare feature values at sampled timestamps
- compare signal outputs on same events
- compare simulated vs live execution assumptions

---

