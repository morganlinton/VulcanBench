import { SuitesShell } from "@/components/suites/SuitesShell";
import { TaskReview } from "@/components/suites/TaskReview";

export const dynamic = "force-dynamic";

interface Props {
  params: Promise<{ id: string; task: string }>;
}

function decode(v: string): string {
  try {
    return decodeURIComponent(v);
  } catch {
    return v;
  }
}

export default async function TaskPage({ params }: Props) {
  const { id, task } = await params;
  const suite = decode(id);
  return (
    <SuitesShell back={{ href: `/suites/${encodeURIComponent(suite)}`, label: "suite" }}>
      <TaskReview suite={suite} task={decode(task)} />
    </SuitesShell>
  );
}
