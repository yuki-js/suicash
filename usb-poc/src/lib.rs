//! Also exposes usb-poc as a library so the CLI (main.rs) and the payment terminal
//! daemon (bin/facepay.rs) share card / oracle.

pub mod card;
pub mod oracle;
