pub mod telemetry;

//
// Telemetry is a mechanism used to capture metrics in an application,
// and either store the data locally or upload to a server for
// statistical analysis.
//
// Examples of usage:
// - capturing the speed of an operation;
// - finding out if users are actually using a feature;
// - finding out how the duration of a session;
// - determine the operating system on which the application is executed;
// - determining the configuration of the application;
// - capturing the operations that slow down the application;
// - determining the amount of I/O performed by the application;
// - ...
//
// The abstraction used by this library is the Histogram. Each
// Histogram serves to capture a specific measurement, store it
// locally and/or upload it to the server. Several types of Histograms
// are provided, suited to distinct kinds of measures.
//

