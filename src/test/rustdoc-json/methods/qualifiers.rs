// edition:2018

pub struct Foo;

impl Foo {
    // @is qualifiers.json "$.index[*][?(@.name=='const_meth')].inner.header.async" false
    // @is - "$.index[*][?(@.name=='const_meth')].inner.header.const"  true
    // @is - "$.index[*][?(@.name=='const_meth')].inner.header.unsafe" false
    pub const fn const_meth() {}

    // @is - "$.index[*][?(@.name=='nothing_meth')].inner.header.async"  false
    // @is - "$.index[*][?(@.name=='nothing_meth')].inner.header.const"  false
    // @is - "$.index[*][?(@.name=='nothing_meth')].inner.header.unsafe" false
    pub fn nothing_meth() {}

    // @is - "$.index[*][?(@.name=='unsafe_meth')].inner.header.async"  false
    // @is - "$.index[*][?(@.name=='unsafe_meth')].inner.header.const"  false
    // @is - "$.index[*][?(@.name=='unsafe_meth')].inner.header.unsafe" true
    pub unsafe fn unsafe_meth() {}

    // @is - "$.index[*][?(@.name=='async_meth')].inner.header.async"  true
    // @is - "$.index[*][?(@.name=='async_meth')].inner.header.const"  false
    // @is - "$.index[*][?(@.name=='async_meth')].inner.header.unsafe" false
    pub async fn async_meth() {}

    // @is - "$.index[*][?(@.name=='async_unsafe_meth')].inner.header.async"  true
    // @is - "$.index[*][?(@.name=='async_unsafe_meth')].inner.header.const"  false
    // @is - "$.index[*][?(@.name=='async_unsafe_meth')].inner.header.unsafe" true
    pub async unsafe fn async_unsafe_meth() {}

    // @is - "$.index[*][?(@.name=='const_unsafe_meth')].inner.header.async"  false
    // @is - "$.index[*][?(@.name=='const_unsafe_meth')].inner.header.const"  true
    // @is - "$.index[*][?(@.name=='const_unsafe_meth')].inner.header.unsafe" true
    pub const unsafe fn const_unsafe_meth() {}

    // It's impossible for a method to be both const and async, so no test for that
}
