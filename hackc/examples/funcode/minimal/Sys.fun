class Sys {
  static sum: number;

  static init(): number {
    let i: number = 0;
    let v: Vector = new Vector(3, 4);
    Sys.sum = 1;
    while (i < 4) {
      i = i + 1;
      Sys.sum = Sys.add(Sys.sum, Sys.sum);
    }
    return Sys.sum;
  }

  static add(a: number, b: number): number {
    return a + b;
  }
}

class Vector {
  x: number;
  y: number;
  constructor(x: number, y: number) {
    this.x = x;
    this.y = y;
  }
}

class Memory {
  static end: number;
  static alloc(size: number): number {
    while (Memory.end == 0) {
      Memory.end = 1000;
    }
    let pointer: number = Memory.end;
    Memory.end = Memory.end + size;
    return pointer;
  }
}
