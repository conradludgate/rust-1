// run-rustfix
#![deny(warnings)]
#![feature(doc_notable_trait)]

#[doc(notable_trait)]
//~^ ERROR unknown `doc` attribute `notable_trait`
//~| WARN this was previously accepted by the compiler but is being phased out; it will become a hard error in a future release!
trait MyTrait {}
