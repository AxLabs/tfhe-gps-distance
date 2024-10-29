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

```
   Copyright 2024 AxLabs

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
```