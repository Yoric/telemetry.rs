//! Telemetry is a mechanism used to capture metrics in an application,
//! to later store the data locally or upload it to a server for
//! statistical analysis.
//!
//!
//! Examples of usage:
//!
//! - capturing the speed of an operation;
//! - finding out if a remote service is often down, and how much impact this has on users;
//! - finding out if users are actually using a feature;
//! - finding out how the duration of a session;
//! - determine the operating system on which the application is executed;
//! - determining the configuration of the application;
//! - capturing the operations that slow down the application;
//! - determining the amount of I/O performed by the application;
//! - ...
//!
//!
//! This crate provides an API for recording such data in _Histograms_
//! and then serializing the data. Uploading the data or storing th
//! data is out of the scope of this crate.
//!
//!
//!
//! Memory note: the memory used by a histogram is recollected only
//! when its instance of `telemetry::Service` is garbage-collected. In other
//! words, if a histogram goes out of scope for some reason, its data
//! remains in telemetry and will be stored and/or uploaded in
//! accordance with the configuration of this telemetry instance.
//!
//! See [Mozilla Telemetry
//! Server](https://github.com/mozilla/telemetry-server) for an
//! open-source implementation of a server implementing the Telemetry
//! protocol.

extern crate rustc_serialize;

mod misc;

/// Data that may be converted to numbers for storage in a histogram.
pub use misc::Flatten;

/// A serialization format, as a subset of Json.
pub use misc::SerializationFormat;

/// A subset of data to export.
pub use misc::Subset;

mod indexing;

mod task;

/// Definition of plain histograms, for data in a set known at compile-time.
pub mod plain;

/// Plain histograms.
pub use plain::Histogram;

/// Definition of keyed histograms, for data in a set known dynamically.
pub mod keyed;

/// Keyed histograms.
pub use keyed::KeyedHistogram;

mod service;

/// The Telemetry Service. You need one (or more) per application.
pub use service::Service;
