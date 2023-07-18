use anyhow::anyhow;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::filter::threshold::ThresholdFilter;
use proton_api_rs::log::LevelFilter;
use std::path::Path;

pub fn init_log(file_path: impl AsRef<Path>) -> Result<(), anyhow::Error> {
    let console = ConsoleAppender::builder().target(Target::Stdout).build();
    let log_file = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("[{l}][{d}][{t}]: {m}{n}")))
        .append(true)
        .build(
            file_path.as_ref().join("yhm.log"),
            Box::new(CompoundPolicy::new(
                Box::new(SizeTrigger::new(5 * 1024 * 1024)),
                Box::new(
                    FixedWindowRoller::builder()
                        .base(0)
                        .build(&file_path.as_ref().join("yhm.{}.log").to_string_lossy(), 2)
                        .map_err(|e| anyhow!("Failed to init window roller: {e}"))?,
                ),
            )),
        )
        .map_err(|e| anyhow!("Failed to build file logger: {e}"))?;

    let config = log4rs::Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(LevelFilter::Info)))
                .build("console", Box::new(console)),
        )
        .appender(Appender::builder().build("logfile", Box::new(log_file)))
        .build(
            Root::builder()
                .appender("console")
                .appender("logfile")
                .build(LevelFilter::Debug),
        )
        .map_err(|e| anyhow!("Failed to build log config: {e}"))?;

    log4rs::init_config(config).map_err(|e| anyhow!("Failed to init logger: {e}"))?;
    Ok(())
}
