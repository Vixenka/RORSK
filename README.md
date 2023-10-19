# RORSK
A simple program to compare and emilinate arithmetic calculation differences for 32-bit integers and 32-bit floating-point numbers between devices using Vulkan API.

## [rorsk_generator](/rorsk_generator/)
Program which generates comformant and uncomformant data for current used Vulkan API driver.

## [rorsk_comparer](/rorsk_comparer/)
Program which compares previous generated data by [rorks_generator](/rorsk_generator/) and output results in the console.

## Building and running
To compile this two programs you will need [Rust 1.72](https://www.rust-lang.org/learn/get-started). And when you have it, you just run:
```
cargo build
```
in their directories.

> [!NOTE]
> For running [rorks_generator](/rorsk_generator/) you must have a Vulkan API driver.

And for run you must:
```
cargo run
```
also in their directories.

## Legal notes
RORSK is licensed under the [MIT](/LICENSE) license and was created under the action "NAKOLATEK - Nastoletni Naukowiec" funded by Minister Education and Science of government of the Republic of Poland as part of VIA CARPATIA polytechnic network named after President of Republic of Poland Lech Kaczyński.