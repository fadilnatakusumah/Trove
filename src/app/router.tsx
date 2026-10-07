import { createBrowserRouter } from "react-router-dom";
import { DashboardPage } from "@/features/dashboard/DashboardPage";
import { ProjectsPage } from "@/features/projects/ProjectsPage";
import { ProvidersPage } from "@/features/providers/ProvidersPage";
import { SettingsPage } from "@/features/settings/SettingsPage";
import { SkillDetailPage } from "@/features/skills/SkillDetailPage";
import { SkillsPage } from "@/features/skills/SkillsPage";
import { RootLayout } from "@/components/layout/RootLayout";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <RootLayout />,
    children: [
      { index: true, element: <DashboardPage /> },
      { path: "skills", element: <SkillsPage /> },
      { path: "skills/:skillId", element: <SkillDetailPage /> },
      { path: "projects", element: <ProjectsPage /> },
      { path: "providers", element: <ProvidersPage /> },
      { path: "settings", element: <SettingsPage /> },
    ],
  },
]);
