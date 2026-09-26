//! usb-poc をライブラリとしても公開し、CLI(main.rs)と決済端末デーモン
//! (bin/facepay.rs)で card / oracle を共有する。

pub mod card;
pub mod oracle;
