import { Route, Routes } from "react-router-dom";
import MainLayout from "@/components/Layout/MainLayout";
import Dashboard from "@/pages/Dashboard";
import Missions from "@/pages/Missions";
import Tasks from "@/pages/Tasks";
import Approvals from "@/pages/Approvals";
import Messaging from "@/pages/Messaging";
import Files from "@/pages/Files";
import Settings from "@/pages/Settings";

/**
 * Root layout + route table for the ONYX desktop shell. Originally a
 * deliberately small set of routes matching Team Prompt 5's own page
 * list (Dashboard/Missions/Tasks/Approvals — an operations console, not
 * a marketing site, so no landing page, no auth screens). Phase 1
 * (Desktop & Web Completion) adds Messaging, Files, and Settings, all
 * backed by already-wired `client-composition` command/query registries
 * (`communication-domain`'s Conversation/Message/ConnectionRequest,
 * `file-domain`'s FileAsset/UploadSession, `policy-domain`'s
 * Policy/LegalHold — see `DECISIONS.md`). Still no real auth flow (see
 * `desktop-shell/src/lib.rs`'s own `TODO(auth/org resolution)` note).
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
        <Route path="/messaging" element={<Messaging />} />
        <Route path="/messaging/:conversationId" element={<Messaging />} />
        <Route path="/files" element={<Files />} />
        <Route path="/files/:fileAssetId" element={<Files />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/settings/:policyId" element={<Settings />} />
      </Routes>
    </MainLayout>
  );
}
