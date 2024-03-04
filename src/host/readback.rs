use super::*;

impl Host {
  /// Creates an ast tree from a wire in a runtime net.
  pub fn readback_tree(&self, wire: &run::Wire) -> Tree {
    ReadbackState { host: self, vars: Default::default(), var_id: 0 .. }.read_wire(wire.clone())
  }

  /// Creates an ast net from a runtime net.
  ///
  /// Note that vicious circles and disconnected subnets will not be in the
  /// resulting ast net, as it is impossible to read these back from the runtime
  /// net representation. In the case of vicious circles, this may result in
  /// unbound variables.
  pub fn readback<M: Mode>(&self, rt_net: &run::Net<M>) -> Net {
    let mut state = ReadbackState { host: self, vars: Default::default(), var_id: 0 .. };
    let mut net = Net::default();

    net.root = state.read_wire(rt_net.root.clone());

    for (a, b) in &rt_net.redexes {
      net.redexes.push((state.read_port(a.clone(), None), state.read_port(b.clone(), None)))
    }

    net
  }
}
/// See [`Host::readback`].
struct ReadbackState<'a> {
  host: &'a Host,
  vars: HashMap<Addr, usize>,
  var_id: RangeFrom<usize>,
}

impl<'a> ReadbackState<'a> {
  /// Reads a tree out from a given `wire`.
  fn read_wire(&mut self, wire: Wire) -> Tree {
    let port = wire.load_target();
    self.read_port(port, Some(wire))
  }
  /// Reads a tree out from a given `port`. If this is a var port, the
  /// `wire` this port was reached from must be supplied to key into the
  /// `vars` map.
  fn read_port(&mut self, port: Port, wire: Option<Wire>) -> Tree {
    match port.tag() {
      Tag::Var => {
        let key = wire.unwrap().addr().min(port.addr());
        Tree::Var {
          nam: create_var(match self.vars.entry(key) {
            Entry::Occupied(e) => e.remove(),
            Entry::Vacant(e) => *e.insert(self.var_id.next().unwrap()),
          }),
        }
      }
      Tag::Red => self.read_wire(port.wire()),
      Tag::Ref if port == Port::ERA => Tree::Era,
      Tag::Ref => Tree::Ref { nam: self.host.back[&port.addr()].clone() },
      Tag::Num => Tree::Num { val: port.num() },
      Tag::Op2 => {
        let opr = port.op();
        let node = port.traverse_node();
        Tree::Op2 { opr, lft: Box::new(self.read_wire(node.p1)), rgt: Box::new(self.read_wire(node.p2)) }
      }
      Tag::Op1 => {
        let opr = port.op();
        let node = port.traverse_op1();
        Tree::Op1 { opr, lft: node.num.num(), rgt: Box::new(self.read_wire(node.p2)) }
      }
      Tag::Ctr => {
        let node = port.traverse_node();
        Tree::Ctr { lab: node.lab, lft: Box::new(self.read_wire(node.p1)), rgt: Box::new(self.read_wire(node.p2)) }
      }
      Tag::Mat => {
        let node = port.traverse_node();
        Tree::Mat { sel: Box::new(self.read_wire(node.p1)), ret: Box::new(self.read_wire(node.p2)) }
      }
    }
  }
}