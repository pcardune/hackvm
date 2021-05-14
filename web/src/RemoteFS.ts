export type FetchState = {
  loading: boolean;
  error: Response | null;
  data: string;
  url: string;
};

type RemoteFSSubscription = (s: FetchState) => void;

export default class RemoteFS {
  private static instance: RemoteFS;
  static get() {
    if (!RemoteFS.instance) {
      RemoteFS.instance = new RemoteFS();
    }
    return RemoteFS.instance;
  }

  private state: Record<string, FetchState> = {};
  private subscribers: Record<string, RemoteFSSubscription[]> = {};

  private constructor() {}

  private notifySubscribers(url: string) {
    const callbacks = this.subscribers[url] || [];
    for (const callback of callbacks) {
      callback({ ...this.state[url] });
    }
  }

  subscribe(url: string, callback: RemoteFSSubscription) {
    if (!this.subscribers[url]) {
      this.subscribers[url] = [];
    }

    if (this.subscribers[url].indexOf(callback) === -1) {
      this.subscribers[url].push(callback);
      callback(this.state[url]);
    }
  }

  unsubscribe(url: string, callback: RemoteFSSubscription) {
    if (this.subscribers[url]) {
      this.subscribers[url] = this.subscribers[url].filter(
        (cb) => cb !== callback
      );
    }
  }

  addFile(url: string, callback?: RemoteFSSubscription) {
    if (!this.state[url]) {
      this.state[url] = { loading: true, error: null, url, data: "" };
      fetch(url).then((res) => {
        if (res.ok) {
          res.text().then((data) => {
            this.state[url] = { loading: false, error: null, url, data };
            this.notifySubscribers(url);
          });
        } else {
          this.state[url] = { loading: false, error: res, url, data: "" };
          this.notifySubscribers(url);
        }
      });
    }

    if (callback) {
      this.subscribe(url, callback);
    }

    return this.state[url];
  }

  async getFile(url: string): Promise<FetchState> {
    return new Promise((resolve, reject) => {
      const callback = (file: FetchState) => {
        if (!file.loading) {
          if (!file.error) {
            resolve(file);
            this.unsubscribe(url, callback);
          } else {
            reject(file);
            this.unsubscribe(url, callback);
          }
        }
      };
      this.addFile(url, callback);
    });
  }

  async getFiles(urls: string[]): Promise<FetchState[]> {
    let files: FetchState[] = [];
    for (const url of urls) {
      const file = await this.getFile(url);
      files.push(file);
    }
    return files;
  }
}
