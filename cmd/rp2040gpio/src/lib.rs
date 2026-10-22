use humility_cli::{ExecutionContext, humility_cmd};
use humility_hiffy::*;

use anyhow::{Result, bail};
use clap::Parser;
use hif::*;

use std::convert::TryInto;

#[derive(Parser, Debug)]
#[clap(name = "rp2040gpio", about = "GPIO pin manipulation (rp2040 variant)")]
pub struct GpioArgs {
    /// sets timeout
    #[clap(
        long, short = 'T', default_value_t = 5000, value_name = "timeout_ms",
        value_parser = parse_int::parse::<u64>
    )]
    timeout: u64,

    /// read the value of the specified pin
    #[clap(
        long, short,
        conflicts_with_all = &["toggle", "set", "reset", "direction"]
    )]
    input: bool,

    /// toggle specified pins
    #[clap(
        long, short, requires = "pins",
        conflicts_with_all = &["set", "reset", "direction"]
    )]
    toggle: bool,

    /// sets specified pins
    #[clap(
        long, short, requires = "pins",
        conflicts_with_all = &["reset", "direction"]
    )]
    set: bool,

    /// resets specified pins
    #[clap(long, short, requires = "pins", conflicts_with_all = &["direction"])]
    reset: bool,

    /// configures specified pins
    #[clap(long, short, requires = "pins")]
    direction: Option<Dir>,

    /// specifies GPIO pins on which to operate
    #[clap(long, short, value_name = "pins", use_value_delimiter = true)]
    pins: Option<Vec<String>>,
}

#[derive(Debug, Copy, Clone, PartialEq, strum::EnumString, strum::AsRefStr)]
#[repr(u8)]
enum Dir {
    In = 0,
    Out = 1,
}

fn gpio(subargs: GpioArgs, context: &mut ExecutionContext) -> Result<()> {
    let hubris = &context.cli.archive()?;
    let log = context.log();
    let core = &mut *context.cli.attach_live_booted(hubris)?;
    let timeout = std::time::Duration::from_millis(subargs.timeout);
    let mut context = HiffyContext::new(hubris, core, timeout, log)?;

    // query for the functions before-hand so it fails if they aren't there
    let gpio_toggle = context.get_function("GpioToggle", 1)?;
    let gpio_set = context.get_function("GpioSet", 1)?;
    let gpio_reset = context.get_function("GpioReset", 1)?;
    let gpio_input = context.get_function("GpioInput", 1)?;
    let gpio_direction = context.get_function("GpioDirection", 2)?;

    let target = if subargs.input {
        gpio_input
    } else if subargs.toggle {
        gpio_toggle
    } else if subargs.set {
        gpio_set
    } else if subargs.reset {
        gpio_reset
    } else if subargs.direction.is_some() {
        gpio_direction
    } else {
        bail!(
            "expected one of set, reset, toggle, direction, or input to be specified"
        )
    };

    let pins = match subargs.pins {
        Some(pins) => pins
            .into_iter()
            .map(|pin| {
                let arg = target.lookup_argument(hubris, "pin", 0, &pin)?;
                Ok((pin, arg))
            })
            .collect::<Result<Vec<_>>>()?,
        None if subargs.input => target.argument_variants(hubris, 0)?,
        None => Vec::new(),
    };

    let mut ops = Vec::new();

    for (_, pin_arg) in &pins {
        // All of the functions take the pin as the first argument,
        // and will eventually call target and drop the arguments
        ops.push(Op::Push16(*pin_arg));

        if let Some(dir) = subargs.direction.as_ref() {
            let dir_arg =
                target.lookup_argument(hubris, "Dir", 1, dir.as_ref())?;
            ops.push(Op::Push16(dir_arg));
        }

        ops.push(Op::Call(target.id));
        ops.push(Op::DropN(target.args.len() as u8));
    }

    ops.push(Op::Done);

    let results = context.run(core, ops.as_slice(), None)?;

    if subargs.input {
        for (ndx, (pin, _)) in pins.iter().enumerate() {
            println!(
                "{} = {}",
                pin,
                match results[ndx] {
                    Err(code) => {
                        target.strerror(code)
                    }
                    Ok(ref val) => {
                        let arr: &[u8; 2] = val[0..2].try_into()?;
                        let v = u16::from_le_bytes(*arr);
                        format!("{}", v)
                    }
                }
            );
        }
    } else {
        println!("{results:?}");
    }

    Ok(())
}

humility_cmd!(GpioArgs, gpio);
