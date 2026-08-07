import { Route, Routes } from "react-router-dom";
import MainLayout from "@/components/Layout/MainLayout";
import Dashboard from "@/pages/Dashboard";
import Missions from "@/pages/Missions";
import Tasks from "@/pages/Tasks";
import Approvals from "@/pages/Approvals";

/**
 * Root layout + route table for the ONYX desktop shell. A deliberately
 * small set of routes matching Team Prompt 5's own page list — this is
 * an operations console, not a marketing site, so there is no landing
 * page, no auth screens (no real auth flow exists yet — see
 * `desktop-shell/src/lib.rs`'s `TODO(auth/org resolution)` note), just
 * the four working views.
 */
export default function App() {
  return (
    <MainLayout>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/missions" element={<Missions />} />
        <Route path="/missions/:missionId" element={<Missions />} />
        <Route path="/tasks" element={<Tasks />} />
        <Route path="/tasks/:taskId" element={<Tasks />} />
        <Route path="/approvals" element={<Approvals />} />
      </Routes>
    </MainLayout>
  );
}
