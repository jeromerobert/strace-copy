# About

`strace-copy` copy the files needed for a program, from one prefix to another, using strace.

To create the required strace log files run (see man strace for details):

```
strace -o <log file> -ff -e trace=file,process <command line ...>
```


# Installation

```
cargo install --git https://github.com/jeromerobert/strace-copy.git
```


# Using

```
Usage: strace-copy [OPTIONS] <DESTINATION_PREFIX> [STRACE_LOGS]...

Arguments:
  <DESTINATION_PREFIX>
          Destination prefix

  [STRACE_LOGS]...
          input `strace` log files

Options:
  -v, --verbose
          

      --prefix <PREFIX>
          Source prefix
          
          [default: /usr/]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

