Telemetry.rs
========

[![Build Status](https://api.travis-ci.org/Yoric/telemetry.rs.svg?branch=master)](https://api.travis-ci.org/Yoric/telemetry.rs)

Telemetry is a mechanism used to capture metrics in an application, to later store the data locally or upload it to a server for statistical analysis.



Examples of usage:
- capturing the speed of an operation;
- finding out if users are actually using a feature;
- finding out how the duration of a session;
- determine the operating system on which the application is executed;
- determining the configuration of the application;
- capturing the operations that slow down the application;
- determining the amount of I/O performed by the application;
- ...

The main abstraction used by this library is the Histogram. Each
Histogram serves to capture a specific measurement. Measurements can
then be exported, so that applications can save them to disk or upload
them to a server. Several types of Histograms are provided, suited to
distinct kinds of measures.

API documentation may be found [here](http://yoric.github.io/telemetry.rs/doc/latest/telemetry/).


## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
