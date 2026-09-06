import { DomainProvider } from "./stores/domainStores";
import { AppShell } from "./AppShell";

export function App() {
  return (
    <DomainProvider>
      <AppShell />
    </DomainProvider>
  );
}
