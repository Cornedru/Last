use oxc_allocator::Allocator;
use oxc_ast_visit::VisitMut;

pub struct ProxyFunctions<'a> {
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> ProxyFunctions<'a> {
    pub fn new(_allocator: &'a Allocator) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a> VisitMut<'a> for ProxyFunctions<'a> {}
