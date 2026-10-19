#[cfg(feature = "mail")]
pub mod filesystem;
#[cfg(feature = "mail")]
pub mod mysql;
#[cfg(feature = "mail")]
pub mod offline;

#[cfg(test)]
extern crate self as hmail_cleaner_adapters;

#[cfg(feature = "mail")]
pub mod service;

#[cfg(feature = "mtls")]
pub mod mtls;

pub mod journal;
