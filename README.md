# TFHE GPS Distance

This repository contains a simple project that calculates the distance between GPS coordinates using the [TFHE](https://github.com/zama-ai/tfhe-rs) (Fully Homomorphic Encryption) library.

## Table of Contents

- [Introduction](#introduction)
- [Installation](#installation)
- [Usage](#usage)
- [Contributing](#contributing)
- [License](#license)

## Introduction

This project demonstrates the use of TFHE to securely compute the distance between two GPS coordinates without revealing the actual coordinates.

## Installation

To install the necessary dependencies, run:

```bash
cargo build
```

## Usage

To calculate the distance, use the following command:

```bash
cargo run --release
```

Note that the `--release` is important due to performance, especially when using [TFHE-rs](https://github.com/zama-ai/tfhe-rs) lib.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](LICENSE) file for details.
