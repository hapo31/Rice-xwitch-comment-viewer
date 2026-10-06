import { act, render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { describe, expect, it, vi } from "vitest";
import { createDomainStores, DomainProvider, useQueueSelector } from "./domainStores";
import { createExternalStore, useStoreSelector } from "./store";

type State = { value: number; noise: number };
function makeStore() {
  return createExternalStore<State, Partial<State>>((state, patch) => ({ ...state, ...patch }), {
    value: 1,
    noise: 0,
  });
}
const objectSelector = (state: State) => ({ value: state.value });
const arraySelector = (state: State) => [state.value];

describe("derived external store selectors", () => {
  it.each([false, true])("caches derived objects and arrays (StrictMode=%s)", (strict) => {
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    const store = makeStore();
    let subscriptions = 0;
    const subscribe = store.subscribe;
    store.subscribe = (listener) => {
      subscriptions++;
      const close = subscribe(listener);
      return () => {
        subscriptions--;
        close();
      };
    };
    let selectedObject: { value: number } | undefined;
    let selectedArray: number[] | undefined;
    function Derived({ label }: { label: string }) {
      selectedObject = useStoreSelector(store, objectSelector);
      selectedArray = useStoreSelector(store, arraySelector);
      return (
        <output>
          {label}:{selectedObject.value}:{selectedArray.join(",")}
        </output>
      );
    }
    const tree = (label: string) =>
      strict ? (
        <StrictMode>
          <Derived label={label} />
        </StrictMode>
      ) : (
        <Derived label={label} />
      );
    const view = render(tree("before"));
    const firstObject = selectedObject;
    const firstArray = selectedArray;
    expect(subscriptions).toBe(2);
    view.rerender(tree("after"));
    expect(selectedObject).toBe(firstObject);
    expect(selectedArray).toBe(firstArray);
    act(() => store.dispatch({ value: 2 }));
    expect(screen.getByText("after:2:2")).toBeInTheDocument();
    expect(selectedObject).not.toBe(firstObject);
    expect(selectedArray).not.toBe(firstArray);
    expect(errors).not.toHaveBeenCalled();
    view.unmount();
    expect(subscriptions).toBe(0);
  });

  it("uses the supplied comparison to suppress unrelated updates in the same store", () => {
    const store = makeStore();
    let renders = 0;
    function Derived() {
      renders++;
      const value = useStoreSelector(
        store,
        (state) => ({ value: state.value }),
        (a, b) => a.value === b.value,
      );
      return <output>{value.value}</output>;
    }
    render(<Derived />);
    const before = renders;
    act(() => store.dispatch({ noise: 7 }));
    expect(renders).toBe(before);
    act(() => store.dispatch({ value: 2 }));
    expect(renders).toBe(before + 1);
    expect(screen.getByText("2")).toBeInTheDocument();
  });

  it("reselects when the selector or its captured props change without a store update", () => {
    const store = makeStore();
    function Derived({ multiplier }: { multiplier: number }) {
      const selected = useStoreSelector(
        store,
        (state) => ({ value: state.value * multiplier }),
        (a, b) => a.value === b.value,
      );
      return <output>{selected.value}</output>;
    }
    const view = render(<Derived multiplier={2} />);
    expect(screen.getByText("2")).toBeInTheDocument();
    view.rerender(<Derived multiplier={3} />);
    expect(screen.getByText("3")).toBeInTheDocument();
    act(() => store.dispatch({ value: 4 }));
    expect(screen.getByText("12")).toBeInTheDocument();
  });

  it("switches its subscription when the store changes", () => {
    const first = makeStore();
    const second = makeStore();
    second.dispatch({ value: 2 });
    let renders = 0;
    function Derived({ store }: { store: ReturnType<typeof makeStore> }) {
      renders++;
      return <output>{useStoreSelector(store, objectSelector).value}</output>;
    }
    const view = render(<Derived store={first} />);
    view.rerender(<Derived store={second} />);
    const before = renders;
    act(() => first.dispatch({ value: 9 }));
    expect(renders).toBe(before);
    expect(screen.getByText("2")).toBeInTheDocument();
    act(() => second.dispatch({ value: 3 }));
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("keeps Providers isolated and delivers one domain notification and render per event", () => {
    const first = createDomainStores();
    const second = createDomainStores();
    const notified = vi.fn();
    const close = first.queue.subscribe(notified);
    const renders = { first: 0, second: 0 };
    function QueueCount({ name }: { name: keyof typeof renders }) {
      renders[name]++;
      const selected = useQueueSelector(
        (state) => ({ count: state.items.length }),
        (a, b) => a.count === b.count,
      );
      return (
        <output>
          {name}:{selected.count}
        </output>
      );
    }
    render(
      <>
        <DomainProvider stores={first}>
          <QueueCount name="first" />
        </DomainProvider>
        <DomainProvider stores={second}>
          <QueueCount name="second" />
        </DomainProvider>
      </>,
    );
    act(() =>
      first.queue.dispatch({
        type: "items.replaced",
        revision: 1,
        items: [{ id: "q1", text: "hello", userDisplayName: "viewer", status: "queued" }],
      }),
    );
    expect(notified).toHaveBeenCalledOnce();
    expect(renders).toEqual({ first: 2, second: 1 });
    expect(screen.getByText("first:1")).toBeInTheDocument();
    expect(screen.getByText("second:0")).toBeInTheDocument();
    act(() => first.connection.dispatch({ type: "auth.status.changed", status: "authenticated" }));
    expect(renders).toEqual({ first: 2, second: 1 });
    close();
  });
});
