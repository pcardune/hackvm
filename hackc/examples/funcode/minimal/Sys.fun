class Sys {
  static init(): number {
    let i: number = 0;
    let sum: number = 1;
    while (i < 4) {
      i = i + 1;
      sum = sum + sum;
    }
    return sum;
  }
}
