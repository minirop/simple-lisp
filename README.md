Simple LISP
===========

A basic interpreter and compiler for my toy language. To build, just do `cargo build`.

```console
$ simple-lisp file.sl    # interpreter
$ simple-lisp file.sl -c # compiler (converts it to C++ then call g++)
$ simple-lisp file.sl -e # bytecode (converts it to bytecode for the RockVM¹)
```

There are differences in support from each backend. So not all example will run the same on each (or might even be unsupported).

¹: [RockVM](https://github.com/minirop/rockvm)
