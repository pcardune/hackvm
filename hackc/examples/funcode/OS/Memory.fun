class Memory {
  static heapStart: number;

  static init(): void {
    Memory.heapStart = 2048;
  }
  static peek(addr: number): number {
    let ram: number[] = 0;
    return ram[addr];
  }
  static poke(addr: number, value: number): void {
    let ram: number[] = 0;
    ram[addr] = value;
  }
  static alloc(size: number): number {
    let p: number = Memory.heapStart;
    Memory.heapStart = Memory.heapStart + size;
    return p;
  }
  static deAlloc(): void {
    // todo...
  }
}
