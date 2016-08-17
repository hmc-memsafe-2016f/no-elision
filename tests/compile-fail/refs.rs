#![allow(dead_code)]
#![feature(rustc_attrs)]

fn main() {}

fn id<'a>(x: &'a i32) -> &i32 {
    //~^ ERROR Nice try!
    x
}

struct Ref<'a> {
    r: &'a i32,
}

struct DoubleRef<'a, 'b> {
    x: &'a i32,
    y: &'b i32,
}

fn mkref<'a>(x: &'a i32) -> Ref {
    //~^ ERROR Nice try!
    Ref { r: x }
}

fn mkref_good<'a>(x: &'a i32) -> Ref<'a> {
    Ref { r: x }
}

fn were<'a, F>(x: &'a i32, f: F) -> &i32 where F: Fn(&i32) -> &i32 {
    //~^ ERROR Nice try!
    //~^^ ERROR Nice try!
    //~^^^ ERROR Nice try!
    f(x)
}
