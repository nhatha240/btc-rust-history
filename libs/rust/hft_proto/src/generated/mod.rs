pub mod common {
    include!(concat!(env!("OUT_DIR"), "/common.rs"));
}
pub mod md {
    include!(concat!(env!("OUT_DIR"), "/md.rs"));
}
pub mod ai {
    include!(concat!(env!("OUT_DIR"), "/ai.rs"));
}
pub mod oms {
    include!(concat!(env!("OUT_DIR"), "/oms.rs"));
}
