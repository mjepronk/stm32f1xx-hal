//! Deep Sleep test
#![deny(unsafe_code)]
#![no_main]
#![no_std]

extern crate panic_semihosting;

use stm32f1xx_hal::{
    prelude::*,
    pac,
    sleep::{enter_sleep_mode, SleepMode, SleepModeEntry},
    rtc::Rtc,
};
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into
    // the corresponding HAL structs
    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();
    let mut pwr = dp.PWR;
    let mut backup_domain = rcc.bkp.constrain(dp.BKP, &mut rcc.apb1, &mut pwr);
    let mut rtc = Rtc::rtc(dp.RTC, &mut backup_domain);

    // Determine if we're woken up by WKUP pin or RTC
    let wakeup_flag = pwr.csr.read().wuf().bit();
    if wakeup_flag {
        hprintln!("Woke up.").unwrap();
    } else {
        hprintln!("Cold boot.").unwrap();
    }

    let mut scb = cp.SCB;
    let mut nvic = cp.NVIC;
    let mut exti = dp.EXTI;
    let mut dbgmcu = dp.DBGMCU;

    loop {
        // Do something useful here...

        // Now go to sleep...
        rtc.clear_alarm_flag();
        rtc.set_seconds(0);
        rtc.set_alarm(10);
        hprintln!("Going to sleep for 10 seconds (or until you pull PA0 high).").unwrap();

        enter_sleep_mode(
            SleepMode::Standby,
            SleepModeEntry::WFI,
            true,              // enable wakeup alarm
            true,              // enable wakeup pin
            Some(&mut dbgmcu), // enable sleep debug
            &mut scb,
            &mut pwr,
            &mut nvic,
            &mut exti,
            &mut rcc.apb1);

        // Waking up from Standby will reset the MCU (control returns to main),
        // while waking up from Sleep or Stop mode will continue executing any
        // code here...
        hprintln!("Woke up from Sleep or Stop mode.").unwrap();
    }
}

