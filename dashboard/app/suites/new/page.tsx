import { NewSuiteForm } from "@/components/suites/SuitesList";
import { SuitesShell } from "@/components/suites/SuitesShell";

export const dynamic = "force-dynamic";

export default function NewSuitePage() {
  return (
    <SuitesShell back={{ href: "/suites", label: "eval suites" }}>
      <h1 className="mt-4 text-3xl font-semibold tracking-tight">New eval suite</h1>
      <NewSuiteForm />
    </SuitesShell>
  );
}
