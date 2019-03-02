# Mio-Framed

This library is meant to implement the simple common use-case of sending and receiving length-prefixed messages with mio.

Proposals and requests for API are very welcome.

# TODO

* Tests
* Profiling
* Finish context refactoring
* Stop screwing around with performance before I have profiling and benchmarks
* More context APIs
  * unix_listen
  * tcp_listen
  * unix_connect
  * tcp_connect
  * close
  * ssl?
  * ssh?
    * No universal way to connect to unix socket over ssh
    * Need to rely on helped programs
      * I couldn't get `nc -U socketpath` to work
      * `socat - UNIX-CONNECT:socketpath` works well, but socat isn't very commonly present
      * Maybe best to include small helper program
* Parameterize frame length type (u16/u32/u64)?