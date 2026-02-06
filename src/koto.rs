#![allow(clippy::result_large_err)]

#[cfg(any(feature = "gc", feature = "agc"))]
use std::time::Instant;

use koto::{derive::*, prelude::*, runtime};

#[cfg(not(any(feature = "rc", feature = "gc")))]
use koto::runtime::Result;

#[cfg(any(feature = "rc", feature = "gc"))]
type Rc<T> = std::rc::Rc<T>;

#[cfg(not(any(feature = "rc", feature = "gc")))]
type Rc<T> = std::sync::Arc<T>;

#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, KotoCopy, KotoType, KotoTrace)]
pub struct RustData(pub Rc<str>);

#[koto_impl]
impl RustData {
    fn new_koto_object(s: &str) -> KObject {
        let me = Self(s.into());
        KObject::from(me)
    }
}

impl KotoObject for RustData {
    fn display(&self, ctx: &mut DisplayContext) -> runtime::Result<()> {
        ctx.append(self.0.to_string());
        Ok(())
    }

    fn less(&self, rhs: &KValue) -> runtime::Result<bool> {
        if let KValue::Object(kobj) = rhs {
            let rhs_dc = kobj.cast::<RustData>()?;
            Ok(*self < *rhs_dc)
        } else {
            unexpected_type("RustData object", rhs)
        }
    }
}

#[cfg(all(any(feature = "gc", feature = "agc"), feature = "log"))]
fn time_it<R>(name: &str, f: impl FnOnce() -> R) -> R {
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    println!("{name} finished in {elapsed:?}");
    result
}

pub fn sort_userdata(
    run: impl FnOnce(&mut dyn FnMut()),
    validate: impl FnOnce(KValue),
) -> anyhow::Result<()> {
    #[cfg(feature = "gc")]
    {
        use std::cell::Cell;

        thread_local! {
            static LAST_NOW: Cell<Instant> = Cell::new(Instant::now());
        }

        LAST_NOW.set(Instant::now());
        koto::runtime::memory::dumpster::unsync::set_collect_condition(move |info| {
            let do_collect = false;

            // let do_collect = info.n_gcs_dropped_since_last_collect() > 100_000;

            // let do_collect =
            //     koto::runtime::memory::dumpster::unsync::default_collect_condition(info);

            #[cfg(feature = "log")]
            if do_collect {
                let now = Instant::now();
                let elapsed = now.duration_since(LAST_NOW.get());
                LAST_NOW.set(now);
                eprintln!(
                    "collecting garbage after {elapsed:?} existing={} dropped={}",
                    info.n_gcs_existing(),
                    info.n_gcs_dropped_since_last_collect(),
                );
            }

            do_collect
        });
    }

    #[cfg(feature = "agc")]
    {
        use std::sync::Mutex;

        static LAST_NOW: Mutex<Option<Instant>> = Mutex::new(None);

        *LAST_NOW.lock().unwrap() = Some(Instant::now());
        koto::runtime::memory::dumpster::sync::set_collect_condition(move |info| {
            let do_collect = false;

            // let do_collect = info.n_gcs_dropped_since_last_collect() > 100_000;

            // let do_collect = koto::runtime::memory::dumpster::sync::default_collect_condition(info);

            #[cfg(feature = "log")]
            if do_collect {
                let now = Instant::now();
                let elapsed = now.duration_since(LAST_NOW.lock().unwrap().unwrap());
                *LAST_NOW.lock().unwrap() = Some(now);
                eprintln!(
                    "collecting garbage after {elapsed:?} existing={} dropped={}",
                    info.n_gcs_existing(),
                    info.n_gcs_dropped_since_last_collect(),
                );
            }

            do_collect
        });
    }

    let mut engine = Koto::default();
    let prelude = engine.prelude();

    prelude.add_fn("RustData_new", |ctx| match ctx.args() {
        [KValue::Str(input)] => Ok(RustData::new_koto_object(input.as_str()).into()),
        unexpected => unexpected_args("a string", unexpected),
    });

    prelude.add_fn("rand", |ctx| match ctx.args() {
        [KValue::Number(n)] => {
            let res = rand::random::<u32>() as i64 % i64::from(n);
            Ok(res.into())
        }
        unexpected => unexpected_args("a number", unexpected),
    });

    engine.compile_and_run(include_str!("../scripts/sort_userdata.koto"))?;
    let Some(bench) = engine.exports().get("bench") else {
        anyhow::bail!("Missing bench function");
    };

    validate(engine.call_function(bench.clone(), &[]).unwrap());

    run(&mut || {
        engine.call_function(bench.clone(), &[]).unwrap();

        #[cfg(any(feature = "gc", feature = "agc"))]
        {
            #[cfg(feature = "log")]
            time_it("collect", koto::runtime::memory::collect_garbage);

            #[cfg(not(feature = "log"))]
            koto::runtime::memory::collect_garbage();
        }
    });

    Ok(())
}
