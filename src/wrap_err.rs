// Copyright 2020 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

macro_rules! wrap_err {
    ($x:expr, $y:path $(, $z:expr)* $(,)?) => {{
        match $x {
            Ok(v) => {
                v
            },
            Err(e) => {
                return Err($y(e $(, $z)*));
            },
        }
    }}
}
