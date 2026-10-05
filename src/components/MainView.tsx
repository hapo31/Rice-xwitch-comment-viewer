import { memo, useEffect } from "react";
import { Navigate, Route, Routes, useLocation, useNavigationType } from "react-router-dom";
import {
  DomainAuthView,
  DomainChatView,
  DomainFilterView,
  DomainLauncherView,
  DomainLogsView,
  DomainQueueView,
  DomainSettingsView,
} from "../features/domainViews";
import {
  getRouteDocumentTitle,
  routeHeadingId,
  shouldFocusRouteHeading,
} from "../routeAccessibility";

interface MainViewProps {
  showStartupGuide: boolean;
}

export const MainView = memo(function MainView({ showStartupGuide }: MainViewProps) {
  const location = useLocation();
  const navigationType = useNavigationType();

  useEffect(() => {
    document.title = getRouteDocumentTitle(location.pathname);

    if (shouldFocusRouteHeading(navigationType)) {
      document.getElementById(routeHeadingId)?.focus();
    }
  }, [location.pathname, navigationType]);

  return (
    <Routes>
      <Route path="/" element={<Navigate to="/chat" replace />} />
      <Route path="/chat" element={<DomainChatView showStartupGuide={showStartupGuide} />} />
      <Route path="/queue" element={<DomainQueueView />} />
      <Route path="/launcher" element={<DomainLauncherView />} />
      <Route path="/filter" element={<DomainFilterView />} />
      <Route path="/rules" element={<Navigate to="/filter" replace />} />
      <Route path="/settings" element={<DomainSettingsView />} />
      <Route path="/voices" element={<Navigate to="/settings" replace />} />
      <Route path="/auth" element={<DomainAuthView />} />
      <Route path="/logs" element={<DomainLogsView />} />
      <Route path="*" element={<Navigate to="/chat" replace />} />
    </Routes>
  );
});
