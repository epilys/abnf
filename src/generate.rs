use std::{
    collections::{HashMap, VecDeque},
    convert::TryFrom,
    sync::Arc,
};

use arbitrary::{Result, Unstructured};
use rand::{rand_core::UnwrapErr, TryRng};

use crate::types::{Node, Repeat, Rule, TerminalValues};

pub struct UnstructuredRng<'b, 'a: 'b>(pub &'b mut Unstructured<'a>);

impl<'a, 'b: 'a> TryRng for UnstructuredRng<'a, 'b> {
    type Error = arbitrary::Error;

    fn try_next_u32(&mut self) -> Result<u32> {
        self.0.arbitrary::<u32>()
    }

    fn try_next_u64(&mut self) -> Result<u64> {
        self.0.arbitrary::<u64>()
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<()> {
        self.0.fill_buffer(dst)
    }
}

#[derive(Clone)]
pub struct OptionalAdjust {
    pub matches: Arc<dyn Fn(&str, &Node) -> bool>,
    pub sample: Arc<dyn Fn(&str, &mut Unstructured<'_>) -> bool>,
}

#[derive(Clone)]
pub struct AlternativesAdjust {
    pub matches: Arc<dyn Fn(&str, &[Node]) -> bool>,
    pub sample: Arc<dyn for<'a> Fn(&str, &'a [Node], &mut Unstructured<'_>) -> &'a Node>,
}

#[derive(Clone)]
pub struct RepeatAdjust {
    pub matches: Arc<dyn Fn(&str, &Node) -> bool>,
    pub sample: Arc<
        dyn for<'a> Fn(
            &str,
            &Node,
            // min
            Option<usize>,
            // max
            Option<usize>,
            &mut Unstructured<'_>,
        ) -> core::ops::RangeInclusive<u32>,
    >,
}

pub struct Generator {
    pub ruleset: HashMap<String, Rule>,
    pub opt_adjustments: Vec<OptionalAdjust>,
    pub alt_adjustments: Vec<AlternativesAdjust>,
    pub repeat_adjustments: Vec<RepeatAdjust>,
}

impl Generator {
    pub fn arbitrary_data<'a>(&self, root: &str, u: &mut Unstructured<'a>) -> Result<String> {
        let mut queue = VecDeque::new();
        queue.push_back((root, self.ruleset[root].node()));

        let mut output = String::new();

        while !u.is_empty() {
            let Some((rulename, node)) = queue.pop_front() else {
                break;
            };

            match node {
                Node::Alternatives(nodes) => {
                    let node = &nodes[u.choose_index(nodes.len())?];
                    queue.push_front((rulename, node));
                }
                Node::Concatenation(nodes) => {
                    for node in nodes.iter().rev() {
                        queue.push_front((rulename, node));
                    }
                }
                Node::Repetition { repeat, node } => match repeat {
                    Repeat::Specific(r) => {
                        for _ in 0..*r {
                            queue.push_front((rulename, node));
                        }
                    }
                    Repeat::Variable { min, max } => {
                        if let Some(adjustment) = self
                            .repeat_adjustments
                            .iter()
                            .find(|adj| (adj.matches)(rulename, node.as_ref()))
                        {
                            for _ in (adjustment.sample)(rulename, node.as_ref(), *min, *max, u) {
                                queue.push_front((rulename, node));
                            }
                        } else {
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
                            let len = u.int_in_range(min..=max)?;
                            for _ in min..(min + len) {
                                queue.push_front((rulename, node));
                            }
                        }
                    }
                },
                Node::Rulename(rulename) => {
                    queue.push_front((rulename, self.ruleset[rulename].node()));
                }
                Node::Group(node) => {
                    queue.push_front((rulename, node));
                }
                Node::Optional(node) => {
                    if let Some(adjustment) = self
                        .opt_adjustments
                        .iter()
                        .find(|adj| (adj.matches)(rulename, node.as_ref()))
                    {
                        if (adjustment.sample)(rulename, u) {
                            queue.push_front((rulename, node));
                        }
                    } else if u.arbitrary::<bool>()? {
                        queue.push_front((rulename, node));
                    }
                }
                Node::String(string_literal) => {
                    output.push_str(string_literal.as_str());
                }
                Node::TerminalValues(terminal_values) => match terminal_values {
                    TerminalValues::Range(start, end) => {
                        output.push(char::try_from(u.int_in_range(*start..=*end)?).unwrap());
                    }
                    TerminalValues::Concatenation(items) => {
                        output.extend(items.iter().map(|c| char::try_from(*c).unwrap()));
                    }
                },
                Node::Prose(prose) => {
                    output.push_str(prose);
                }
            }
        }

        Ok(output)
    }
}
