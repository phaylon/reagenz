use std::sync::Arc;

use crate::World;
use crate::system::{System, SystemSymbolError};
use crate::value::Value;


pub(crate) fn load_core_system<W>(mut sys: System<W>) -> Result<System<W>, SystemSymbolError>
where
    W: World,
{
    sys.register_node("is-symbol", |_, [v]| v.is_symbol().into()).unwrap();
    sys.register_node("is-int", |_, [v]| v.is_int().into()).unwrap();
    sys.register_node("is-float", |_, [v]| v.is_float().into()).unwrap();
    sys.register_node("is-external", |_, [v]| v.is_ext().into()).unwrap();
    sys.register_node("is-list", |_, [v]| v.is_list().into()).unwrap();

    sys.register_query("list-items", |_, [list]| {
        if let Value::List(values) = list {
            Box::new(ListIter::new(values.clone()))
        } else {
            Box::new(std::iter::empty())
        }
    }).unwrap();

    sys.register_node("symbol-in-list", |_, [a, b]| {
        use Value::{Symbol, List};
        if let (symbol @ Symbol(_), List(list)) | (List(list), symbol @ Symbol(_)) = (a, b) {
            list.contains(symbol).into()
        } else {
            false.into()
        }
    }).unwrap();

    sys.register_node("symbols=", |_, [a,b]| {
        if let(Value::Symbol(a),Value::Symbol(b)) = (a,b) {
            (a==b).into()
        } else {
            false.into()
        }
    }).unwrap();

    sys.register_node("fail", |_, []| false.into()).unwrap();
    sys.register_node("ok", |_, []| true.into()).unwrap();

    Ok(sys)
}

struct ListIter<W: World> {
    values: Arc<[Value<W>]>,
    next_index: usize,
}

impl<W> ListIter<W>
where
    W: World,
{
    fn new(values: Arc<[Value<W>]>) -> Self {
        Self { values, next_index: 0 }
    }
}

impl<W> Iterator for ListIter<W>
where
    W: World,
{
    type Item = Value<W>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.values.get(self.next_index)?.clone();
        self.next_index += 1;
        Some(item)
    }
}