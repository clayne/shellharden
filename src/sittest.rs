/*
 * Copyright 2021-2022 Andreas Nordal
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use crate::situation::COLOR_NORMAL;
use crate::situation::Horizon;
use crate::situation::Situation;
use crate::situation::Transition;
use crate::situation::WhatNow;
use crate::situation::flush;
use crate::situation::flush_or_pop;
use crate::situation::push;
use crate::situation::COLOR_CMD;

use crate::commonargcmd::common_arg;
use crate::commonargcmd::common_token;
use crate::machine::expression_tracker;
use crate::microparsers::is_word;
use crate::microparsers::prefixlen;

use crate::sitcmd::SitArg;

pub struct SitTest {
	pub end_trigger :u16,
}

impl Situation for SitTest {
	fn whatnow(&mut self, horizon: Horizon) -> WhatNow {
		if horizon.input.len() >= 4 {
			let is_emptystringtest = prefixlen(horizon.input, b"-z ") == 3;
			let is_nonemptystringtest = prefixlen(horizon.input, b"-n ") == 3;
			if is_emptystringtest || is_nonemptystringtest {
				let suggest = common_token(self.end_trigger, horizon, 3);
				if let Some(ref exciting) = suggest {
					if let Transition::Push(_) = &exciting.transition {
						let end_replace: &'static [u8] = if is_emptystringtest {
							b" = \"\""
						} else {
							b" != \"\""
						};
						return push_hiddentest(suggest, end_replace, self.end_trigger);
					} else if horizon.is_lengthenable {
						return flush(0);
					}
				}
			} else if prefixlen(horizon.input, b"x") == 1 {
				if let Some(mut suggest) = common_token(self.end_trigger, horizon, 1) {
					if let Transition::Push(_) = &suggest.transition {
						let transition = std::mem::replace(&mut suggest.transition, Transition::Flush);
						if let Transition::Push(state) = transition {
							let (pre, len, _) = suggest.transform;
							let progress = pre + len;
							if let Ok(found) = find_xyes_comparison(&horizon.input[progress ..], state) {
								if found {
									return push_xyes(self.end_trigger);
								}
								if horizon.is_lengthenable {
									return flush(0);
								}
							}
						}
					} else {
						return suggest;
					}
				}
			}
		} else if horizon.is_lengthenable {
			return flush(0);
		}
		become_regular(self.end_trigger)
	}
	fn get_color(&self) -> u32 {
		COLOR_CMD
	}
}

fn become_regular(end_trigger :u16) -> WhatNow {
	become_regular_with((0, 0, None), end_trigger)
}

fn become_regular_with(
	transform: (usize, usize, Option<&'static [u8]>),
	end_trigger :u16,
) -> WhatNow {
	WhatNow {
		transform,
		transition: Transition::Replace(Box::new(SitArg { end_trigger })),
	}
}

fn push_hiddentest(
	inner: Option<WhatNow>,
	end_replace: &'static [u8],
	end_trigger: u16,
) -> WhatNow {
	push(
		(0, 3, Some(b"")),
		Box::new(SitHiddenTest {
			inner,
			end_replace,
			end_trigger,
		}),
	)
}

fn push_xyes(end_trigger: u16) -> WhatNow {
	push((0, 1, Some(b"")), Box::new(SitXyes { end_trigger }))
}

struct SitHiddenTest {
	inner: Option<WhatNow>,
	end_replace: &'static [u8],
	end_trigger: u16,
}

impl Situation for SitHiddenTest {
	fn whatnow(&mut self, _horizon: Horizon) -> WhatNow {
		let initial_adventure = self.inner.take();
		if let Some(mut exciting) = initial_adventure {
			exciting.transform.0 = 0;
			exciting
		} else {
			become_regular_with((0, 0, Some(self.end_replace)), self.end_trigger)
		}
	}
	fn get_color(&self) -> u32 {
		COLOR_NORMAL
	}
}

struct SitXyes {
	end_trigger :u16,
}

impl Situation for SitXyes {
	fn whatnow(&mut self, horizon: Horizon) -> WhatNow {
		for (i, &a) in horizon.input.iter().enumerate() {
			if a == b'x' {
				let mut replacement: &'static [u8] = b"\"\"";
				if i+1 < horizon.input.len() {
					if is_word(horizon.input[i+1]) {
						replacement = b"";
					}
				} else if i > 0 || horizon.is_lengthenable {
					return flush(i);
				}
				return become_regular_with((i, 1, Some(replacement)), self.end_trigger);
			}
			if let Some(res) = common_arg(self.end_trigger, horizon, i) {
				return res;
			}
		}
		flush_or_pop(horizon.input.len())
	}
	fn get_color(&self) -> u32 {
		COLOR_NORMAL
	}
}

fn find_xyes_comparison(horizon: &[u8], state: Box<dyn Situation>) -> Result<bool, ()> {
	let (found, exprlen) = expression_tracker(horizon, state)?;
	let after = &horizon[exprlen ..];

	Ok(found && has_rhs_xyes(after))
}

fn has_rhs_xyes(horizon: &[u8]) -> bool {
	#[derive(Clone)]
	#[derive(Copy)]
	enum Lex {
		Start,
		FirstSpace,
		Negation,
		FirstEq,
		SecondEq,
		SecondSpace,
	}
	let mut state = Lex::Start;

	for byte in horizon {
		match (state, byte) {
			(Lex::Start, b' ') => state = Lex::FirstSpace,
			(Lex::FirstSpace, b'=') => state = Lex::FirstEq,
			(Lex::FirstSpace, b'!') => state = Lex::Negation,
			(Lex::Negation, b'=') => state = Lex::SecondEq,
			(Lex::FirstEq, b'=') => state = Lex::SecondEq,
			(Lex::FirstEq, b' ') => state = Lex::SecondSpace,
			(Lex::SecondEq, b' ') => state = Lex::SecondSpace,
			(Lex::SecondSpace, b'x') => return true,
			(_, _) => break,
		}
	}
	false
}

#[cfg(test)]
use crate::testhelpers::*;
#[cfg(test)]
use crate::situation::pop;

#[test]
fn test_sit_test() {
	let subj = || SitTest { end_trigger: 0u16 };

	sit_expect!(subj(), b"", &flush(0), &become_regular(0u16));

	sit_expect!(subj(), b"-f $are ", &become_regular(0u16));
	sit_expect!(subj(), b"-z $are ", &push_hiddentest(None, b"", 0u16));
	sit_expect!(subj(), b"-n $are ", &push_hiddentest(None, b"", 0u16));
	sit_expect!(subj(), b"-z justkidding ", &become_regular(0u16));
	sit_expect!(subj(), b"-n justkidding ", &become_regular(0u16));
	sit_expect!(subj(), b"-z \"", &push_hiddentest(None, b"", 0u16));
	sit_expect!(subj(), b"-n \"", &push_hiddentest(None, b"", 0u16));
	sit_expect!(subj(), b"-n \0", &flush(0), &become_regular(0u16));

	sit_expect!(subj(), b"x   ", &become_regular(0u16));
	sit_expect!(subj(), b"x\0 = x", &pop(1, 0, None));
	sit_expect!(subj(), b"x$( ", &flush(0), &become_regular(0u16));
	sit_expect!(subj(), b"x\"$(echo)\" = ", &flush(0), &become_regular(0u16));
	sit_expect!(subj(), b"x\"$(echo)\" = x", &push_xyes(0u16));
	sit_expect!(subj(), b"x$(echo) = x", &push_xyes(0u16));
	sit_expect!(subj(), b"x`echo` == x", &push_xyes(0u16));
	sit_expect!(subj(), b"x\"$yes\" != x", &push_xyes(0u16));
	sit_expect!(subj(), b"x$yes = x",  &push_xyes(0x16));
	sit_expect!(subj(), b"x$yes = y", &flush(0), &become_regular(0u16));
	sit_expect!(subj(), b"$yes = x", &become_regular(0u16));
	sit_expect!(subj(), b"x$yes = x$1", &push_xyes(0x16));
	sit_expect!(subj(), b"x`$10` = x", &become_regular(0u16));
}

#[test]
fn test_sit_xyes() {
	let subj = || SitXyes { end_trigger: 0u16 };

	sit_expect!(subj(), b" = ", &flush_or_pop(3));
	sit_expect!(subj(), b" = x", &flush(3));
	sit_expect!(subj(), b"x", &flush(0), &become_regular_with((0, 1, Some(b"\"\"")), 0u16));
	sit_expect!(subj(), b" = x ", &become_regular_with((3, 1, Some(b"\"\"")), 0u16));
	sit_expect!(subj(), b" = x;", &become_regular_with((3, 1, Some(b"\"\"")), 0u16));
	sit_expect!(subj(), b" = xx", &become_regular_with((3, 1, Some(b"")), 0u16));
}

#[test]
fn test_has_rhs_xyes() {
	assert!(has_rhs_xyes(b" = x"));
	assert!(has_rhs_xyes(b" != x"));
	assert!(has_rhs_xyes(b" == x"));
	assert!(!has_rhs_xyes(b" = "));
	assert!(!has_rhs_xyes(b" = y"));
	assert!(!has_rhs_xyes(b"= x"));
	assert!(!has_rhs_xyes(b" =x"));
	assert!(!has_rhs_xyes(b"  x"));
	assert!(!has_rhs_xyes(b" ! x"));
}
