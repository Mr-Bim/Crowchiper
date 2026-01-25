/**
 * Minimal reactive primitive for state management.
 *
 * Signals provide a simple way to create reactive state with subscriptions.
 * This is intentionally minimal (~30 lines) to avoid framework complexity.
 *
 * @example
 * ```ts
 * const count = signal(0);
 * count.subscribe((value) => console.log("Count:", value));
 * count.set(1); // logs "Count: 1"
 * count.update((n) => n + 1); // logs "Count: 2"
 * console.log(count.get()); // 2
 * ```
 */

export type Subscriber<T> = (value: T) => void;
export type Unsubscribe = () => void;

export interface Signal<T> {
  /** Get the current value */
  get(): T;
  /** Set a new value and notify subscribers */
  set(value: T): void;
  /** Update the value using a function and notify subscribers */
  update(fn: (current: T) => T): void;
  /** Subscribe to value changes. Returns unsubscribe function. */
  subscribe(fn: Subscriber<T>): Unsubscribe;
}

export interface ReadonlySignal<T> {
  /** Get the current value */
  get(): T;
  /** Subscribe to value changes. Returns unsubscribe function. */
  subscribe(fn: Subscriber<T>): Unsubscribe;
}

/**
 * Create a reactive signal with the given initial value.
 */
export function signal<T>(initial: T): Signal<T> {
  let value = initial;
  const subscribers = new Set<Subscriber<T>>();

  return {
    get() {
      return value;
    },
    set(newValue: T) {
      if (newValue !== value) {
        value = newValue;
        for (const fn of subscribers) {
          fn(value);
        }
      }
    },
    update(fn: (current: T) => T) {
      this.set(fn(value));
    },
    subscribe(fn: Subscriber<T>): Unsubscribe {
      subscribers.add(fn);
      return () => subscribers.delete(fn);
    },
  };
}

/**
 * Create a computed signal that derives its value from other signals.
 *
 * @example
 * ```ts
 * const firstName = signal("John");
 * const lastName = signal("Doe");
 * const fullName = computed(
 *   [firstName, lastName],
 *   ([first, last]) => `${first} ${last}`
 * );
 * console.log(fullName.get()); // "John Doe"
 * ```
 */
export function computed<T, D extends readonly Signal<unknown>[]>(
  deps: D,
  fn: (values: { [K in keyof D]: D[K] extends Signal<infer V> ? V : never }) => T,
): ReadonlySignal<T> {
  const getValues = () =>
    deps.map((d) => d.get()) as {
      [K in keyof D]: D[K] extends Signal<infer V> ? V : never;
    };

  let value = fn(getValues());
  const subscribers = new Set<Subscriber<T>>();

  const notify = () => {
    const newValue = fn(getValues());
    if (newValue !== value) {
      value = newValue;
      for (const sub of subscribers) {
        sub(value);
      }
    }
  };

  // Subscribe to all dependencies
  for (const dep of deps) {
    dep.subscribe(notify);
  }

  return {
    get() {
      return value;
    },
    subscribe(fn: Subscriber<T>): Unsubscribe {
      subscribers.add(fn);
      return () => subscribers.delete(fn);
    },
  };
}
