//! Code related to `sqlite3_context` common to `functions` and `vtab` modules.

use libsqlite3_sys::sqlite3_value;
use std::ffi::{c_int, c_void};
#[cfg(feature = "array")]
use std::rc::Rc;

use crate::ffi;
use crate::ffi::sqlite3_context;

use crate::str_for_sqlite;
use crate::types::{ToSqlOutput, ValueRef};
#[cfg(feature = "array")]
use crate::vtab::array::{free_array, ARRAY_TYPE};

// This function is inline despite it's size because what's in the ToSqlOutput
// is often known to the compiler, and thus const prop/DCE can substantially
// simplify the function.
#[inline]
pub(super) unsafe fn set_result(
    ctx: *mut sqlite3_context,
    #[allow(unused_variables)] args: &[*mut sqlite3_value],
    result: &ToSqlOutput<'_>,
) {
    let value = match *result {
        ToSqlOutput::Borrowed(v) => v,
        ToSqlOutput::Owned(ref v) => ValueRef::from(v),

        #[cfg(feature = "blob")]
        ToSqlOutput::ZeroBlob(len) => {
            // TODO sqlite3_result_zeroblob64 // 3.8.11
            return ffi::sqlite3_result_zeroblob(ctx, len);
        }
        #[cfg(feature = "functions")]
        ToSqlOutput::Arg(i) => {
            return ffi::sqlite3_result_value(ctx, args[i]);
        }
        #[cfg(feature = "array")]
        ToSqlOutput::Array(ref a) => {
            return ffi::sqlite3_result_pointer(
                ctx,
                Rc::into_raw(a.clone()) as *mut c_void,
                ARRAY_TYPE,
                Some(free_array),
            );
        }
    };

    match value {
        ValueRef::Null => ffi::sqlite3_result_null(ctx),
        ValueRef::Integer(i) => ffi::sqlite3_result_int64(ctx, i),
        ValueRef::Real(r) => ffi::sqlite3_result_double(ctx, r),
        ValueRef::Text(s) => {
            let length = s.len();
            if length > c_int::MAX as usize {
                ffi::sqlite3_result_error_toobig(ctx);
            } else {
                let Ok((c_str, len, destructor)) = str_for_sqlite(s) else {
                    // TODO sqlite3_result_error
                    return ffi::sqlite3_result_error_code(ctx, ffi::SQLITE_MISUSE);
                };
                // TODO sqlite3_result_text64 // 3.8.7
                ffi::sqlite3_result_text(ctx, c_str, len, destructor);
            }
        }
        ValueRef::Blob(b) => {
            let length = b.len();
            if length > c_int::MAX as usize {
                ffi::sqlite3_result_error_toobig(ctx);
            } else if length == 0 {
                ffi::sqlite3_result_zeroblob(ctx, 0);
            } else {
                // TODO sqlite3_result_blob64 // 3.8.7
                ffi::sqlite3_result_blob(
                    ctx,
                    b.as_ptr().cast::<c_void>(),
                    length as c_int,
                    ffi::SQLITE_TRANSIENT(),
                );
            }
        }
    }
}

#[inline]
#[must_use]
pub(super) unsafe fn no_change(ctx: *mut sqlite3_context) -> bool {
    ffi::sqlite3_vtab_nochange(ctx) != 0
}
