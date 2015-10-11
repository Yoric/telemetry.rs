//! Telemetry is a mechanism used to capture metrics in an application,
//! to later store the data locally or upload it to a server for
//! statistical analysis.
//!
//! Examples of usage:
//!
//! - capturing the speed of an operation;
//! - finding out if users are actually using a feature;
//! - finding out how the duration of a session;
//! - determine the operating system on which the application is executed;
//! - determining the configuration of the application;
//! - capturing the operations that slow down the application;
//! - determining the amount of I/O performed by the application;
//! - ...
//!
//! To make use of Telemetry, an application needs:
//!
//! - an instance of [`telemetry::Service`](services/struct.Service.html), designed
//!   to hold all the state in a dedicated thread and export it as needed;
//! - one or more instances of
//!   [`telemetry::Feature`](services/struct.Feature.html), designed allow activating
//!   or deactivating a group of histograms together;
//! - one or more instances of either
//!   [`single::Histogram`](single/trait.Histogram.html) or
//!   [`keyed::KeyedHistogram`](keyed/trait.KeyedHistogram.html), designed to
//!   actually record the data.
//!
//!
//! Memory note: the memory used by a histogram is recollected only
//! when its instance of `telemetry` is garbage-collected. In other
//! words, if a histogram goes out of scope for some reason, its data
//! remains in telemetry and will be stored and/or uploaded in
//! accordance with the configuration of this telemetry instance.


extern crate rustc_serialize;


mod misc;
pub use misc::{Flatten, SerializationFormat, Version};

mod indexing;

mod task;

pub mod single;
pub mod keyed;
mod service;
pub use service::{Feature, Service};
pub use keyed::KeyedHistogram;
pub use single::Histogram;




