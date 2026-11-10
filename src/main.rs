use std::error::Error;

use abnf::rulelist;

fn main() -> Result<(), Box<dyn Error>> {
    // let rules = {
    //     let path = std::env::args().nth(1).ok_or("No path to file given.")?;
    //     let data = std::fs::read_to_string(path)?;

    //     rulelist(&data)?
    // };

    // for rule in &rules {
    //     println!("// {}", rule);
    //     println!("{:#?}\n", rule);
    // }
    #[cfg(feature = "generate")]
    {
        use abnf::types::{Node, Rule};
        use getrandom::{rand_core::TryRng, SysRng};
        use rand::rand_core::UnwrapErr;
        use std::sync::Arc;

        use abnf::generate::*;

        let rules = {
            let data = include_str!("../examples/assets/rfc5322.abnf");
            rulelist(data)?
        };
        let mut seed = vec![0u8; 15048];
        SysRng.try_fill_bytes(&mut seed).unwrap();
        use arbitrary::Unstructured;
        let core_rules = rulelist(include_str!("../examples/assets/abnf_core.abnf"))?;
        let ruleset = core_rules
            .into_iter()
            .chain(rules)
            .map(|r| (r.name().to_string(), r))
            .collect();
        let mut u = Unstructured::new(&seed);
        let generator = Generator {
            ruleset,
            opt_adjustments: vec![OptionalAdjust {
                matches: Arc::new(|_rulename, node| -> bool {
                    matches!(node, Node::Rulename(ref n) if ["FWS", "CRLF", "comment", "CFWS"].contains(&n.as_str()))
                }),
                sample: Arc::new(|_rulename, u| -> bool {
                    use rand::distr::{Bernoulli, Distribution};

                    let d = Bernoulli::new(0.01).unwrap();
                    d.sample(&mut UnwrapErr(&mut UnstructuredRng(u)))
                }),
            }],
            alt_adjustments: vec![],
            repeat_adjustments: vec![
                RepeatAdjust {
                    matches: Arc::new(|rulename, _node| -> bool { rulename == "fields" }),
                    sample: Arc::new(
                        |_rulename, _n, min, max, u| -> core::ops::RangeInclusive<u32> {
                            use std::convert::TryFrom;
                            let min = min.map(|m| u32::try_from(m).unwrap());
                            let max = max.map(|m| u32::try_from(m).unwrap());
                            let min = min.unwrap_or(0);
                            let max = if let Some(max) = max {
                                max
                            } else {
                                use rand_distr::{Distribution, Poisson};
                                let poi = Poisson::new(10.0).unwrap();
                                let v: f64 = poi.sample(&mut UnwrapErr(&mut UnstructuredRng(u)));
                                v as u32
                            };
                            let len = u.int_in_range(min..=max).unwrap();
                            min..=(min + len)
                        },
                    ),
                },
                RepeatAdjust {
                    matches: Arc::new(|rulename, _node| -> bool {
                        ["FWS", "obs-FWS", "comment", "CFWS"].contains(&rulename)
                    }),
                    sample: Arc::new(
                        |_rulename, _n, min, max, u| -> core::ops::RangeInclusive<u32> {
                            use std::convert::TryFrom;
                            let min = min.map(|m| u32::try_from(m).unwrap());
                            let max = max.map(|m| u32::try_from(m).unwrap());
                            let min = min.unwrap_or(0);
                            let max = if let Some(max) = max {
                                max
                            } else {
                                use rand_distr::{Distribution, Poisson};
                                let poi = Poisson::new(0.5).unwrap();
                                let v: f64 = poi.sample(&mut UnwrapErr(&mut UnstructuredRng(u)));
                                v as u32
                            };
                            let range = min..=max;
                            if range.is_empty() {
                                return range;
                            }
                            let len = u.int_in_range(min..=max).unwrap();
                            min..=(min + len)
                        },
                    ),
                },
            ],
        };
        // let mut u = Unstructured::new(&[1, 2, 3, 4]);
        let root = "message";
        eprintln!("{:?}", generator.arbitrary_data(root, &mut u).unwrap());
    }

    Ok(())
}
