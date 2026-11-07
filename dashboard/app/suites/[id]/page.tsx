import { SuiteDetail } from "@/components/suites/SuiteDetail";
import { SuitesShell } from "@/components/suites/SuitesShell";

export const dynamic = "force-dynamic";

interface Props {
  params: Promise<{ id: string }>;
}

function decode(v: string): string {
  try {
    return decodeURIComponent(v);
  } catch {
    return v;
  }
}

export default async function SuitePage({ params }: Props) {
  const { id } = await params;
  return (
    <SuitesShell back={{ href: "/suites", label: "suites" }}>
      <SuiteDetail id={decode(id)} />
    </SuitesShell>
  );
}
