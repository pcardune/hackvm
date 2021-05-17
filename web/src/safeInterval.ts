export default function safeInterval(func: () => void, delay: number) {
  const interval = setInterval(() => {
    try {
      func();
    } catch (e) {
      clearInterval(interval);
      throw e;
    }
  }, delay);
  return interval;
}
