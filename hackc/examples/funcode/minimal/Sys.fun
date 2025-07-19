class Syscalls {
  static write(
    fileDescriptor: number,
    strPointer: number,
    strLength: number
  ): number {
    return c.syscall(1, fileDescriptor, strPointer, strLength);
  }
}

declare module C {
  function malloc(size: number): number;
  function free(pointer: number): void;
  function memcpy(destPointer: number, srcPointer: number, size: number): void;
  function putchar(c: number): number;
  function puts(c: number): number;
}

class Memory {
  static alloc(size: number): number {
    return C.malloc(size);
  }
}

class String {
  capacity: number;
  length: number;
  chars: number;
  constructor(capacity: number) {
    this.length = 0;
    this.capacity = capacity + 1;
    let chars: number = C.malloc(this.capacity);
    chars[0] = 0;
    this.chars = chars;
  }
  appendChar(c: number): String {
    let chars: number = this.chars;
    if (this.length + 1 + 1 > this.capacity) {
      let newCapacity: number = this.capacity + 2;
      chars = C.malloc(newCapacity);
      C.memcpy(chars, this.chars, this.capacity);
      this.capacity = newCapacity;
      this.chars = chars;
    }
    chars[this.length] = c;
    chars[this.length + 1] = 0;
    this.length = this.length + 1;
    return this;
  }
  add(c: number): void {
    let chars: number = this.chars;
    chars[this.length] = c;
    this.length = this.length + 1;
  }
}

class Sys {
  static sum: number;

  static init(): number {
    let s: String = new String(5);
    let chars: number = s.chars;
    s.add(111);
    // chars[s.length] = 111;
    C.puts(chars);
    C.putchar(104);
    let j: number = C.malloc(1);
    j[0] = 104; // h
    j[1] = 101; // e
    j[2] = 108; // l
    j[3] = 108; // l
    j[4] = 111; // o
    j[5] = 10; // \n
    j[6] = 0; // \0
    C.puts(j);
    Syscalls.write(1, j, 6);
    j[1] = 93;
    j[2] = 94;
    j[3] = 95;
    j[4] = 96;
    return j[0];
  }

  static add(a: number, b: number): number {
    return a + b;
  }
}
