# ADS123x

A `#![no_std]` Rust driver library for interacting with TI [ADS1232](https://www.ti.com/product/ADS1232) and [ADS1234](https://www.ti.com/product/ADS1234) Delta-Sigma ADC chips.

## Cargo Features

All features are disabled by default.

- `defmt` - Implements `defmt::Format` for most public types so they can be printed using `defmt::info!()` and relatives
- `embedded-hal-async` - Provides async implementations of all the ADS123x functions
