// Despite the file name, this is not actually a JIT (yet).

use crate::run;
use crate::ast;

use std::collections::HashMap;

pub fn compile_book(book: &run::Book) -> String {
  let mut code = String::new();

  code.push_str(&format!("use crate::run::{{*}};\n"));
  code.push_str(&format!("\n"));

  for fid in 0 .. book.defs.len() as run::Val {
    if book.defs[fid as usize].node.len() > 0 {
      let name = &ast::val_to_name(fid as u32);
      code.push_str(&format!("pub const F_{:4} : Val = 0x{:06x};\n", name, fid));
    }
  }
  code.push_str(&format!("\n"));

  code.push_str(&format!("impl Net {{\n"));
  code.push_str(&format!("\n"));

  code.push_str(&format!("{}pub fn call_native(&mut self, book: &Book, ptr: Ptr, x: Ptr) -> bool {{\n", ident(1)));
  code.push_str(&format!("{}match ptr.val() {{\n", ident(2)));
  for fid in 0 .. book.defs.len() as run::Val {
    if book.defs[fid as usize].node.len() > 0 {
      let fun = ast::val_to_name(fid);
      code.push_str(&format!("{}F_{} => {{ return self.F_{}(ptr, x); }}\n", ident(3), fun, fun));
    }
  }
  code.push_str(&format!("{}_ => {{ return false; }}\n", ident(3)));
  code.push_str(&format!("{}}}\n", ident(2)));
  code.push_str(&format!("{}}}\n", ident(1)));
  code.push_str(&format!("\n"));

  for fid in 0 .. book.defs.len() as run::Val {
    if book.defs[fid as usize].node.len() > 0 {
      code.push_str(&compile_term(&book, 1, fid));
      code.push_str(&format!("\n"));
    }
  }

  code.push_str(&format!("}}"));

  return code;

}

pub fn ident(tab: usize) -> String {
  return "  ".repeat(tab);
}

pub fn tag(tag: run::Tag) -> &'static str {
  match tag {
    run::VR1 => "VR1",
    run::VR2 => "VR2",
    run::RD1 => "RD1",
    run::RD2 => "RD2",
    run::REF => "REF",
    run::ERA => "ERA",
    run::NUM => "NUM",
    run::OP2 => "OP2",
    run::OP1 => "OP1",
    run::MAT => "MAT",
    run::CT0 => "CT0",
    run::CT1 => "CT1",
    run::CT2 => "CT2",
    run::CT3 => "CT3",
    run::CT4 => "CT4",
    run::CT5 => "CT5",
    _ => unreachable!(),
  }
}

pub fn atom(ptr: run::Ptr) -> String {
  if ptr.is_ref() {
    return format!("Ptr::new(REF, F_{})", ast::val_to_name(ptr.val()));
  } else {
    return format!("Ptr::new({}, 0x{:x})", tag(ptr.tag()), ptr.val());
  }
}

enum Target {
  Var { nam: String },
  //Val { ptr: run:: Ptr },
}

fn target(trg: &Target) -> String {
  match trg {
    Target::Var { nam } => format!("{}", nam),
  }
}

pub fn compile_term(book: &run::Book, tab: usize, fid: run::Val) -> String {

  // returns a fresh variable: 'v<NUM>'
  fn fresh(newx: &mut usize) -> String {
    *newx += 1;
    format!("k{}", newx)
  }

  fn call(
    book : &run::Book,
    tab  : usize,
    newx : &mut usize,
    vars : &mut HashMap<run::Ptr, String>,
    fid  : run::Val,
    x    : &Target,
  ) -> String {
    //let newx = &mut 0;
    //let vars = &mut HashMap::new();
    let def = &book.defs[fid as usize];
    let mut code = String::new();
    code.push_str(&burn(book, tab, newx, vars, def, def.node[0].1, &x));
    for (rf, rx) in &def.rdex {
      let (rf, rx) = adjust_redex(*rf, *rx);
      let rf_name = format!("_{}", fresh(newx));
      code.push_str(&format!("{}let {} = {};\n", ident(tab), rf_name, &atom(rf)));
      code.push_str(&burn(book, tab, newx, vars, def, rx, &Target::Var { nam: rf_name }));
      //code.push_str(&make(tab, newx, vars, def, rx, &atom(rf)));
    }

    return code;
  }
  
  // @loop = (?<(#0 (x y)) R> R) & @loop ~ (x y)
  fn burn(
    book : &run::Book,
    tab  : usize,
    newx : &mut usize,
    vars : &mut HashMap<run::Ptr, String>,
    def  : &run::Def,
    ptr  : run::Ptr,
    x    : &Target,
  ) -> String {
    //println!("burn {:08x} {}", ptr.0, x);
    let mut code = String::new();

    // (<?(ifz ifs) ret> ret) ~ (#X R)
    // ------------------------------- fast match
    // if X == 0:
    //   ifz ~ R
    //   ifs ~ *
    // else:
    //   ifz ~ *
    //   ifs ~ (#(X-1) R)
    // When ifs is REF, tail-call optimization is applied.
    if ptr.tag() == run::CT0 {
      let mat = def.node[ptr.val() as usize].0;
      let rty = def.node[ptr.val() as usize].1;
      if mat.tag() == run::MAT {
        let cse = def.node[mat.val() as usize].0;
        let rtx = def.node[mat.val() as usize].1;
        let got = def.node[rty.val() as usize];
        let rtz = if rty.tag() == run::VR1 { got.0 } else { got.1 };
        if cse.tag() == run::CT0 && rtx.is_var() && rtx == rtz {
          let ifz = def.node[cse.val() as usize].0;
          let ifs = def.node[cse.val() as usize].1;
          let c_z = Target::Var { nam: fresh(newx) };
          let c_s = Target::Var { nam: fresh(newx) };
          let num = format!("{}x", target(x));
          let res = format!("{}y", target(x));
          let lam = fresh(newx);
          let mat = fresh(newx);
          let cse = fresh(newx);
          code.push_str(&format!("{}let {} : Ptr;\n", ident(tab), target(&c_z)));
          code.push_str(&format!("{}let {} : Ptr;\n", ident(tab), target(&c_s)));
          code.push_str(&format!("{}// fast match\n", ident(tab)));
          code.push_str(&format!("{}if {}.tag() == CT0 && self.heap.get({}.val(), P1).is_num() {{\n", ident(tab), target(x), target(x)));
          code.push_str(&format!("{}self.anni += 2;\n", ident(tab+1)));
          code.push_str(&format!("{}self.oper += 1;\n", ident(tab+1)));
          code.push_str(&format!("{}let {} = self.heap.get({}.val(), P1);\n", ident(tab+1), num, target(x)));
          code.push_str(&format!("{}let {} = self.heap.get({}.val(), P2);\n", ident(tab+1), res, target(x)));
          code.push_str(&format!("{}if {}.val() == 0 {{\n", ident(tab+1), num));
          code.push_str(&format!("{}self.free({}.val());\n", ident(tab+2), target(x)));
          code.push_str(&format!("{}{} = {};\n", ident(tab+2), target(&c_z), res));
          code.push_str(&format!("{}{} = {};\n", ident(tab+2), target(&c_s), "ERAS"));
          code.push_str(&format!("{}}} else {{\n", ident(tab+1)));
          code.push_str(&format!("{}self.heap.set({}.val(), P1, Ptr::new(NUM, {}.val() - 1));\n", ident(tab+2), target(x), num));
          code.push_str(&format!("{}{} = {};\n", ident(tab+2), target(&c_z), "ERAS"));
          code.push_str(&format!("{}{} = {};\n", ident(tab+2), target(&c_s), target(x)));
          code.push_str(&format!("{}}}\n", ident(tab+1)));
          code.push_str(&format!("{}}} else {{\n", ident(tab)));
          code.push_str(&format!("{}let {} = self.alloc(1);\n", ident(tab+1), lam));
          code.push_str(&format!("{}let {} = self.alloc(1);\n", ident(tab+1), mat));
          code.push_str(&format!("{}let {} = self.alloc(1);\n", ident(tab+1), cse));
          code.push_str(&format!("{}self.heap.set({}, P1, Ptr::new(MAT, {}));\n", ident(tab+1), lam, mat));
          code.push_str(&format!("{}self.heap.set({}, P2, Ptr::new(VR2, {}));\n", ident(tab+1), lam, mat));
          code.push_str(&format!("{}self.heap.set({}, P1, Ptr::new(CT0, {}));\n", ident(tab+1), mat, cse));
          code.push_str(&format!("{}self.heap.set({}, P2, Ptr::new(VR2, {}));\n", ident(tab+1), mat, lam));
          code.push_str(&format!("{}self.link(Ptr::new(CT0, {}), {});\n", ident(tab+1), lam, target(x)));
          code.push_str(&format!("{}{} = Ptr::new(VR1, {});\n", ident(tab+1), target(&c_z), cse));
          code.push_str(&format!("{}{} = Ptr::new(VR2, {});\n", ident(tab+1), target(&c_s), cse));
          code.push_str(&format!("{}}}\n", ident(tab)));
          code.push_str(&burn(book, tab, newx, vars, def, ifz, &c_z));
          code.push_str(&burn(book, tab, newx, vars, def, ifs, &c_s));
          return code;
        }
      }
    }

    // <x <y r>> ~ #N
    // --------------------- fast op
    // r <~ #(op(op(N,x),y))
    if ptr.is_op2() {
      let v_x = def.node[ptr.val() as usize].0;
      let cnt = def.node[ptr.val() as usize].1;
      if cnt.is_op2() {
        let v_y = def.node[cnt.val() as usize].0;
        let ret = def.node[cnt.val() as usize].1;
        if let (Some(v_x), Some(v_y)) = (got(vars, def, v_x), got(vars, def, v_y)) {
          let nxt = Target::Var { nam: fresh(newx) };
          let opx = fresh(newx);
          let opy = fresh(newx);
          code.push_str(&format!("{}let {} : Ptr;\n", ident(tab), target(&nxt)));
          code.push_str(&format!("{}// fast op\n", ident(tab)));
          code.push_str(&format!("{}if {}.is_num() && {}.is_num() && {}.is_num() {{\n", ident(tab), target(x), v_x, v_y));
          code.push_str(&format!("{}self.oper += 4;\n", ident(tab+1))); // OP2 + OP1 + OP2 + OP1
          code.push_str(&format!("{}{} = Ptr::new(NUM, self.op(self.op({}.val(),{}.val()),{}.val()));\n", ident(tab+1), target(&nxt), target(x), v_x, v_y));
          code.push_str(&format!("{}}} else {{\n", ident(tab)));
          code.push_str(&format!("{}let {} = self.alloc(1);\n", ident(tab+1), opx));
          code.push_str(&format!("{}let {} = self.alloc(1);\n", ident(tab+1), opy));
          code.push_str(&format!("{}self.heap.set({}, P2, Ptr::new(OP2, {}));\n", ident(tab+1), opx, opy));
          code.push_str(&format!("{}self.link(Ptr::new(VR1,{}), {});\n", ident(tab+1), opx, v_x));
          code.push_str(&format!("{}self.link(Ptr::new(VR1,{}), {});\n", ident(tab+1), opy, v_y));
          code.push_str(&format!("{}self.link(Ptr::new(OP2,{}), {});\n", ident(tab+1), opx, target(x)));
          code.push_str(&format!("{}{} = Ptr::new(VR2, {});\n", ident(tab+1), target(&nxt), opy));
          code.push_str(&format!("{}}}\n", ident(tab)));
          code.push_str(&burn(book, tab, newx, vars, def, ret, &nxt));
          return code;
        }
      }
    }

    // {p1 p2} <~ #N
    // ------------- fast copy
    // p1 <~ #N
    // p2 <~ #N
    if ptr.is_ctr() && ptr.tag() > run::CT0 {
      let x1 = Target::Var { nam: format!("{}x", target(x)) };
      let x2 = Target::Var { nam: format!("{}y", target(x)) };
      let p1 = def.node[ptr.val() as usize].0;
      let p2 = def.node[ptr.val() as usize].1;
      let lc = fresh(newx);
      code.push_str(&format!("{}let {} : Ptr;\n", ident(tab), target(&x1)));
      code.push_str(&format!("{}let {} : Ptr;\n", ident(tab), target(&x2)));
      code.push_str(&format!("{}// fast copy\n", ident(tab)));
      code.push_str(&format!("{}if {}.tag() == NUM {{\n", ident(tab), target(x)));
      code.push_str(&format!("{}self.comm += 1;\n", ident(tab+1)));
      code.push_str(&format!("{}{} = {};\n", ident(tab+1), target(&x1), target(x)));
      code.push_str(&format!("{}{} = {};\n", ident(tab+1), target(&x2), target(x)));
      code.push_str(&format!("{}}} else {{\n", ident(tab)));
      code.push_str(&format!("{}let {} = self.alloc(1);\n", ident(tab+1), lc));
      code.push_str(&format!("{}{} = Ptr::new(VR1, {});\n", ident(tab+1), target(&x1), lc));
      code.push_str(&format!("{}{} = Ptr::new(VR2, {});\n", ident(tab+1), target(&x2), lc));
      code.push_str(&format!("{}self.link(Ptr::new({}, {}), {});\n", ident(tab+1), tag(ptr.tag()), lc, target(x)));
      code.push_str(&format!("{}}}\n", ident(tab)));
      code.push_str(&burn(book, tab, newx, vars, def, p1, &x1));
      code.push_str(&burn(book, tab, newx, vars, def, p2, &x2));
      return code;
    }

    // (p1 p2) <~ (x1 x2)
    // ------------------ fast apply
    // p1 <~ x1
    // p2 <~ x2
    if ptr.is_ctr() && ptr.tag() == run::CT0 {
      let x1 = Target::Var { nam: format!("{}x", target(x)) };
      let x2 = Target::Var { nam: format!("{}y", target(x)) };
      let p1 = def.node[ptr.val() as usize].0;
      let p2 = def.node[ptr.val() as usize].1;
      let lc = fresh(newx);
      code.push_str(&format!("{}let {} : Ptr;\n", ident(tab), target(&x1)));
      code.push_str(&format!("{}let {} : Ptr;\n", ident(tab), target(&x2)));
      code.push_str(&format!("{}// fast apply\n", ident(tab)));
      code.push_str(&format!("{}if {}.tag() == {} {{\n", ident(tab), target(x), tag(ptr.tag())));
      code.push_str(&format!("{}self.anni += 1;\n", ident(tab+1)));
      code.push_str(&format!("{}{} = self.heap.get({}.val(), P1);\n", ident(tab+1), target(&x1), target(x)));
      code.push_str(&format!("{}{} = self.heap.get({}.val(), P2);\n", ident(tab+1), target(&x2), target(x)));
      code.push_str(&format!("{}self.free({}.val());\n", ident(tab+1), target(x)));
      code.push_str(&format!("{}}} else {{\n", ident(tab)));
      code.push_str(&format!("{}let {} = self.alloc(1);\n", ident(tab+1), lc));
      code.push_str(&format!("{}{} = Ptr::new(VR1, {});\n", ident(tab+1), target(&x1), lc));
      code.push_str(&format!("{}{} = Ptr::new(VR2, {});\n", ident(tab+1), target(&x2), lc));
      code.push_str(&format!("{}self.link(Ptr::new({}, {}), {});\n", ident(tab+1), tag(ptr.tag()), lc, target(x)));
      code.push_str(&format!("{}}}\n", ident(tab)));
      code.push_str(&burn(book, tab, newx, vars, def, p1, &x1));
      code.push_str(&burn(book, tab, newx, vars, def, p2, &x2));
      return code;
    }

    // TODO: implement inlining correctly
    // NOTE: enabling this makes dec_bits_tree hang; investigate
    //if ptr.is_ref() {
      //code.push_str(&format!("{}// inline @{}\n", ident(tab), ast::val_to_name(ptr.val())));
      //code.push_str(&format!("{}if !{}.is_skp() {{\n", ident(tab), target(x)));
      //code.push_str(&format!("{}self.dref += 1;\n", ident(tab+1)));
      //code.push_str(&call(book, tab+1, newx, &mut HashMap::new(), ptr.val(), x));
      //code.push_str(&format!("{}}} else {{\n", ident(tab)));
      //code.push_str(&make(tab+1, newx, vars, def, ptr, &target(x)));
      //code.push_str(&format!("{}}}\n", ident(tab)));
      //return code;
    //}

    // ATOM <~ *
    // --------- fast erase
    // nothing
    if ptr.is_num() || ptr.is_era() {
      code.push_str(&format!("{}// fast erase\n", ident(tab)));
      code.push_str(&format!("{}if {}.is_skp() {{\n", ident(tab), target(x)));
      code.push_str(&format!("{}self.eras += 1;\n", ident(tab+1)));
      code.push_str(&format!("{}}} else {{\n", ident(tab)));
      code.push_str(&make(tab+1, newx, vars, def, ptr, &target(x)));
      code.push_str(&format!("{}}}\n", ident(tab)));
      return code;
    }

    code.push_str(&make(tab, newx, vars, def, ptr, &target(x)));
    return code;
  }

  fn make(
    tab  : usize,
    newx : &mut usize,
    vars : &mut HashMap<run::Ptr, String>,
    def  : &run::Def,
    ptr  : run::Ptr,
    x    : &String,
  ) -> String {
    //println!("make {:08x} {}", ptr.0, x);
    let mut code = String::new();
    if ptr.is_nod() {
      let lc = fresh(newx);
      let p1 = def.node[ptr.val() as usize].0;
      let p2 = def.node[ptr.val() as usize].1;
      code.push_str(&format!("{}let {} = self.alloc(1);\n", ident(tab), lc));
      code.push_str(&make(tab, newx, vars, def, p1, &format!("Ptr::new(VR1, {})", lc)));
      code.push_str(&make(tab, newx, vars, def, p2, &format!("Ptr::new(VR2, {})", lc)));
      code.push_str(&format!("{}self.link(Ptr::new({}, {}), {});\n", ident(tab), tag(ptr.tag()), lc, x));
    } else if ptr.is_var() {
      match got(vars, def, ptr) {
        None => {
          //println!("-var fst");
          vars.insert(ptr, x.clone());
        },
        Some(got) => {
          //println!("-var snd");
          code.push_str(&format!("{}self.link({}, {});\n", ident(tab), x, got));
        }
      }
    } else {
      code.push_str(&format!("{}self.link({}, {});\n", ident(tab), x, atom(ptr)));
    }
    return code;
  }

  fn got(
    vars : &HashMap<run::Ptr, String>,
    def  : &run::Def,
    ptr  : run::Ptr,
  ) -> Option<String> {
    if ptr.is_var() {
      let got = def.node[ptr.val() as usize];
      let slf = if ptr.tag() == run::VR1 { got.0 } else { got.1 };
      return vars.get(&slf).cloned();
    } else {
      return None;
    }
  }

  let fun = ast::val_to_name(fid);

  let mut code = String::new();
  code.push_str(&format!("{}pub fn F_{}(&mut self, ptr: Ptr, x: Ptr) -> bool {{\n", ident(tab), fun));
  code.push_str(&call(book, tab+1, &mut 0, &mut HashMap::new(), fid, &Target::Var { nam: "x".to_string() }));
  code.push_str(&format!("{}return true;\n", ident(tab+1)));
  code.push_str(&format!("{}}}\n", ident(tab)));

  return code;
}

// TODO: HVM-Lang must always output in this form.
fn adjust_redex(rf: run::Ptr, rx: run::Ptr) -> (run::Ptr, run::Ptr) {
  if rf.is_skp() && !rx.is_skp() {
    return (rf, rx);
  } else if !rf.is_skp() && rx.is_skp() {
    return (rx, rf);
  } else {
    println!("Invalid redex. Compiled HVM requires that ALL defs are in the form:");
    println!("@name = ROOT");
    println!("  & ATOM ~ TERM");
    println!("  & ATOM ~ TERM");
    println!("  & ATOM ~ TERM");
    println!("  ...");
    println!("Where ATOM must be either a ref (`@foo`), a num (`#123`), or an era (`*`).");
    println!("If you used HVM-Lang, please report on https://github.com/HigherOrderCO/hvm-lang.");
    panic!("Invalid HVMC file.");
  }
}