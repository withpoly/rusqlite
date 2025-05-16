//! Ensure Virtual tables can be declared outside `rusqlite` crate.

#[cfg(feature = "vtab")]
#[test]
fn test_dummy_module() -> rusqlite::Result<()> {
    use rusqlite::vtab::{
        eponymous_only_module, sqlite3_vtab, sqlite3_vtab_cursor, Context, IndexInfo, VTab,
        VTabConnection, VTabCursor, Values,
    };
    use rusqlite::{version_number, Connection, Result};
    use std::marker::PhantomData;
    use std::os::raw::c_int;

    let module = eponymous_only_module::<DummyTab>();

    #[repr(C)]
    struct DummyTab {
        /// Base class. Must be first
        base: sqlite3_vtab,
    }

    unsafe impl<'vtab> VTab<'vtab> for DummyTab {
        type Aux = ();
        type Cursor = DummyTabCursor<'vtab>;

        fn connect(
            _: &mut VTabConnection,
            _aux: Option<&()>,
            _args: &[&[u8]],
        ) -> Result<(String, Self)> {
            let vtab = Self {
                base: sqlite3_vtab::default(),
            };
            Ok(("CREATE TABLE x(value)".to_owned(), vtab))
        }

        fn best_index(&self, info: &mut IndexInfo) -> Result<()> {
            // We approve the ability to use the IN constraint here
            for (i, _) in info.constraints().enumerate() {
                if info.has_in_constraint(i) {
                    info.handle_in_constraint(i, true);
                }
            }
            for (i, (_, mut u)) in info.constraints_and_usages().enumerate() {
                u.set_argv_index(i as i32 + 1);
                u.set_omit(true);
            }
            info.set_estimated_cost(1.);
            Ok(())
        }

        fn open(&'vtab mut self) -> Result<DummyTabCursor<'vtab>> {
            Ok(DummyTabCursor::default())
        }
    }

    #[derive(Default)]
    #[repr(C)]
    struct DummyTabCursor<'vtab> {
        /// Base class. Must be first
        base: sqlite3_vtab_cursor,
        /// The rowid
        row_id: i64,
        phantom: PhantomData<&'vtab DummyTab>,
    }

    unsafe impl VTabCursor for DummyTabCursor<'_> {
        fn filter(
            &mut self,
            _idx_num: c_int,
            _idx_str: Option<&str>,
            args: &Values<'_>,
        ) -> Result<()> {
            // Because we are doing an IN query where the row ids are 0,1,2, etc.,
            // we can simply assert that the value is equal to the index.
            for (i, value) in args.in_values(0).enumerate() {
                assert_eq!(i as i64, value.as_i64()?);
            }
            Ok(())
        }

        fn next(&mut self) -> Result<()> {
            self.row_id += 1;
            Ok(())
        }

        fn eof(&self) -> bool {
            self.row_id > 2
        }

        fn column(&self, ctx: &mut Context, _: c_int) -> Result<()> {
            ctx.set_result(&self.row_id)
        }

        fn rowid(&self) -> Result<i64> {
            Ok(self.row_id)
        }
    }

    let db = Connection::open_in_memory()?;

    db.create_module::<DummyTab>("dummy", module, None)?;

    let version = version_number();
    if version < 3_009_000 {
        return Ok(());
    }

    // Important: generate a sequence of 0..=2
    let mut s = db.prepare("SELECT rowid FROM dummy() WHERE rowid IN (0,1,2)")?;
    let mut dummy = s.query_map([], |row| row.get::<_, i32>(0))?;
    for i in 0..=2 {
        assert_eq!(i, dummy.next().unwrap().unwrap());
    }
    Ok(())
}
