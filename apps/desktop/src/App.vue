<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  AlertCircle,
  Boxes,
  CheckCircle2,
  Code2,
  FileText,
  GitBranch,
  Play,
  RefreshCw,
  Route,
  ShieldCheck,
  Terminal,
} from '@lucide/vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Textarea } from '@/components/ui/textarea'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'

type Risk = 'read' | 'write' | 'local'

interface WorkflowSummary {
  name: string
  version: string
  path: string
  seedEventCount: number
  handlerCount: number
  effectCount: number
  capabilityCount: number
}

interface Capability {
  id: string
  risk: Risk
  description: string
}

interface PlannedEffect {
  handler_index: number
  effect_index: number
  on: string
  effect_type: string
  capabilities: string[]
}

interface RunPlan {
  workflow_name: string
  workflow_version: string
  required_capabilities: Capability[]
  effects: PlannedEffect[]
}

interface TraceEntry {
  id: string
  sequence: number
  kind: Record<string, unknown>
}

interface RunReport {
  run_id: string
  workflow_name: string
  status: 'succeeded' | 'failed'
  trace: TraceEntry[]
  failure: null | { trace_entry_id: string | null; message: string }
}

const workflows = ref<WorkflowSummary[]>([])
const selectedPath = ref('')
const selectedPlan = ref<RunPlan | null>(null)
const workflowSource = ref('')
const runReport = ref<RunReport | null>(null)
const loading = ref(false)
const running = ref(false)
const error = ref('')

const selectedWorkflow = computed(() =>
  workflows.value.find((workflow) => workflow.path === selectedPath.value),
)

const mutatingCapabilities = computed(
  () =>
    selectedPlan.value?.required_capabilities.filter(
      (capability) => capability.risk === 'write',
    ) ?? [],
)

const traceRows = computed(() => runReport.value?.trace ?? [])

onMounted(() => {
  void refreshWorkflows()
})

async function refreshWorkflows() {
  loading.value = true
  error.value = ''
  try {
    workflows.value = await invoke<WorkflowSummary[]>('list_workflows')
  } catch (cause) {
    workflows.value = fallbackWorkflows()
    error.value = `Tauri backend unavailable; showing scaffold data. ${String(cause)}`
  } finally {
    loading.value = false
  }

  selectedPath.value = workflows.value[0]?.path ?? ''
  if (selectedPath.value) {
    await loadWorkflow()
  }
}

async function selectWorkflow(path: string) {
  selectedPath.value = path
  runReport.value = null
  await loadWorkflow()
}

async function loadWorkflow() {
  await Promise.all([loadPlan(), loadSource()])
}

async function loadPlan() {
  if (!selectedPath.value) return

  loading.value = true
  error.value = ''
  try {
    selectedPlan.value = await invoke<RunPlan>('plan_workflow_file', {
      path: selectedPath.value,
    })
  } catch (cause) {
    selectedPlan.value = fallbackPlan(selectedWorkflow.value)
    error.value = `Unable to load plan from backend. ${String(cause)}`
  } finally {
    loading.value = false
  }
}

async function loadSource() {
  if (!selectedPath.value) return

  try {
    workflowSource.value = await invoke<string>('read_workflow_source', {
      path: selectedPath.value,
    })
  } catch {
    workflowSource.value = fallbackSource(selectedWorkflow.value)
  }
}

async function runSelectedWorkflow() {
  if (!selectedPath.value) return

  running.value = true
  error.value = ''
  try {
    runReport.value = await invoke<RunReport>('run_workflow_file', {
      path: selectedPath.value,
    })
  } catch (cause) {
    error.value = `Run failed before the engine returned a report. ${String(cause)}`
  } finally {
    running.value = false
  }
}

function capabilityTone(risk: Risk) {
  if (risk === 'write') return 'bg-[#ffde59] text-black border-black'
  if (risk === 'read') return 'bg-[#6ee7f9] text-black border-black'
  return 'bg-[#d9f99d] text-black border-black'
}

function traceLabel(entry: TraceEntry) {
  const type = String(entry.kind.type ?? 'trace')
  if (type === 'effect_executed') {
    const effect = entry.kind.effect as { type?: string } | undefined
    return `effect ${effect?.type ?? 'unknown'}`
  }
  if (type === 'event_seeded') return 'seed event'
  if (type === 'event_dequeued') return 'event'
  if (type === 'handler_matched') return `handler ${String(entry.kind.on ?? '')}`
  if (type === 'run_started') return 'run started'
  if (type === 'run_ended') return `run ${String(entry.kind.status ?? '')}`
  return type.replaceAll('_', ' ')
}

function traceDetail(entry: TraceEntry) {
  const type = String(entry.kind.type ?? '')
  if (type === 'event_seeded' || type === 'event_dequeued') {
    const event = entry.kind.event as { event_type?: string } | undefined
    return event?.event_type ?? ''
  }
  if (type === 'effect_executed') {
    const observation = entry.kind.observation as { type?: string } | undefined
    return observation?.type?.replaceAll('_', ' ') ?? ''
  }
  if (type === 'handler_matched') return `on ${String(entry.kind.on ?? '')}`
  return entry.id
}

function fallbackWorkflows(): WorkflowSummary[] {
  return [
    {
      name: 'cross-protocol-smoke',
      version: 'devknife.workflow/v1alpha1',
      path: 'examples/workflows/cross-protocol-smoke.workflow.yaml',
      seedEventCount: 1,
      handlerCount: 7,
      effectCount: 8,
      capabilityCount: 9,
    },
  ]
}

function fallbackPlan(workflow: WorkflowSummary | undefined): RunPlan {
  return {
    workflow_name: workflow?.name ?? 'cross-protocol-smoke',
    workflow_version: workflow?.version ?? 'devknife.workflow/v1alpha1',
    required_capabilities: [
      {
        id: 'network.http.read',
        risk: 'read',
        description: 'Call a REST HTTP endpoint.',
      },
      {
        id: 'network.graphql',
        risk: 'write',
        description: 'Call a GraphQL endpoint.',
      },
      {
        id: 'network.websocket',
        risk: 'write',
        description: 'Open a WebSocket connection and exchange messages.',
      },
      {
        id: 'aws.sns.publish',
        risk: 'write',
        description: 'Publish a message to an SNS topic.',
      },
      {
        id: 'aws.sqs.receive',
        risk: 'read',
        description: 'Receive messages from an SQS queue.',
      },
    ],
    effects: [
      {
        handler_index: 0,
        effect_index: 0,
        on: 'workflow.started',
        effect_type: 'rest',
        capabilities: ['network.http.read'],
      },
      {
        handler_index: 1,
        effect_index: 0,
        on: 'account.loaded',
        effect_type: 'graphql',
        capabilities: ['network.graphql'],
      },
      {
        handler_index: 2,
        effect_index: 0,
        on: 'account.users.loaded',
        effect_type: 'websocket',
        capabilities: ['network.websocket'],
      },
    ],
  }
}

function fallbackSource(workflow: WorkflowSummary | undefined) {
  return `# Source preview is available in the Tauri desktop app.
version: ${workflow?.version ?? 'devknife.workflow/v1alpha1'}
name: ${workflow?.name ?? 'cross-protocol-smoke'}
`
}
</script>

<template>
  <TooltipProvider>
    <main class="min-h-screen bg-background text-foreground">
      <div class="grid min-h-screen grid-cols-[280px_minmax(0,1fr)]">
        <aside class="border-r-4 border-black bg-[#f7f0d8]">
          <div class="flex h-16 items-center gap-3 border-b-4 border-black px-5">
            <div class="grid size-9 place-items-center border-2 border-black bg-[#ff5c8a] shadow-[3px_3px_0_#000]">
              <Boxes class="size-5" />
            </div>
            <div>
              <p class="text-sm font-black uppercase leading-none tracking-normal">devknife</p>
              <p class="text-xs font-semibold text-neutral-700">workflow bench</p>
            </div>
          </div>

          <div class="px-4 py-4">
            <div class="mb-3 flex items-center justify-between">
              <p class="text-xs font-black uppercase tracking-normal text-neutral-700">Workflows</p>
              <Tooltip>
                <TooltipTrigger as-child>
                  <Button
                    variant="outline"
                    size="icon"
                    class="size-8 border-2 border-black bg-white shadow-[2px_2px_0_#000]"
                    :disabled="loading"
                    @click="refreshWorkflows"
                  >
                    <RefreshCw class="size-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Refresh workflow files</TooltipContent>
              </Tooltip>
            </div>

            <ScrollArea class="h-[calc(100vh-132px)] pr-3">
              <button
                v-for="workflow in workflows"
                :key="workflow.path"
                class="mb-3 w-full border-2 border-black bg-white p-3 text-left shadow-[4px_4px_0_#000] transition-transform hover:-translate-y-0.5"
                :class="workflow.path === selectedPath ? 'bg-[#d9f99d]' : ''"
                @click="selectWorkflow(workflow.path)"
              >
                <div class="mb-2 flex items-start justify-between gap-2">
                  <span class="break-words text-sm font-black leading-tight">
                    {{ workflow.name }}
                  </span>
                  <Badge class="border-2 border-black bg-[#6ee7f9] text-black">
                    {{ workflow.effectCount }}
                  </Badge>
                </div>
                <p class="mb-2 break-words font-mono text-[11px] text-neutral-700">
                  {{ workflow.path }}
                </p>
                <div class="flex gap-2 text-[11px] font-bold text-neutral-800">
                  <span>{{ workflow.seedEventCount }} seeds</span>
                  <span>{{ workflow.handlerCount }} handlers</span>
                </div>
              </button>
            </ScrollArea>
          </div>
        </aside>

        <section class="min-w-0 bg-[#fffdf4]">
          <header class="flex h-16 items-center justify-between border-b-4 border-black bg-white px-6">
            <div class="min-w-0">
              <p class="text-xs font-black uppercase tracking-normal text-neutral-600">
                {{ selectedWorkflow?.version ?? 'devknife.workflow/v1alpha1' }}
              </p>
              <h1 class="truncate text-xl font-black tracking-normal">
                {{ selectedWorkflow?.name ?? 'No workflow selected' }}
              </h1>
            </div>
            <div class="flex items-center gap-2">
              <Button
                variant="outline"
                class="border-2 border-black bg-[#6ee7f9] text-black shadow-[3px_3px_0_#000]"
                :disabled="loading || !selectedPath"
                @click="loadPlan"
              >
                <Route class="size-4" />
                Plan
              </Button>
              <Button
                class="border-2 border-black bg-[#ff5c8a] text-black shadow-[3px_3px_0_#000] hover:bg-[#ff7aa0]"
                :disabled="running || !selectedPath"
                @click="runSelectedWorkflow"
              >
                <Play class="size-4" />
                Run
              </Button>
            </div>
          </header>

          <div class="grid gap-4 p-5 xl:grid-cols-[minmax(0,1fr)_360px]">
            <div class="min-w-0">
              <div
                v-if="error"
                class="mb-4 flex gap-3 border-2 border-black bg-[#ffde59] p-3 text-sm font-semibold shadow-[3px_3px_0_#000]"
              >
                <AlertCircle class="mt-0.5 size-4 shrink-0" />
                <span>{{ error }}</span>
              </div>

              <Tabs default-value="plan" class="w-full">
                <TabsList class="mb-4 border-2 border-black bg-white shadow-[3px_3px_0_#000]">
                  <TabsTrigger value="plan">Plan</TabsTrigger>
                  <TabsTrigger value="source">Source</TabsTrigger>
                  <TabsTrigger value="trace">Trace</TabsTrigger>
                  <TabsTrigger value="raw">Raw</TabsTrigger>
                </TabsList>

                <TabsContent value="plan">
                  <Card class="border-4 border-black bg-white shadow-[6px_6px_0_#000]">
                    <CardHeader>
                      <CardTitle class="flex items-center gap-2 text-lg font-black">
                        <ShieldCheck class="size-5" />
                        Required capabilities
                      </CardTitle>
                      <CardDescription>
                        Advisory preflight surface generated by devknife-core.
                      </CardDescription>
                    </CardHeader>
                    <CardContent>
                      <div class="mb-5 flex flex-wrap gap-2">
                        <Badge
                          v-for="capability in selectedPlan?.required_capabilities"
                          :key="capability.id"
                          class="border-2 px-2 py-1 font-mono text-xs"
                          :class="capabilityTone(capability.risk)"
                        >
                          {{ capability.id }}
                        </Badge>
                      </div>

                      <Separator class="mb-4 h-0.5 bg-black" />

                      <Table>
                        <TableHeader>
                          <TableRow>
                            <TableHead class="w-[90px]">Step</TableHead>
                            <TableHead>Handler</TableHead>
                            <TableHead>Effect</TableHead>
                            <TableHead>Capabilities</TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          <TableRow
                            v-for="effect in selectedPlan?.effects"
                            :key="`${effect.handler_index}-${effect.effect_index}`"
                          >
                            <TableCell class="font-mono">
                              {{ effect.handler_index }}.{{ effect.effect_index }}
                            </TableCell>
                            <TableCell class="font-semibold">{{ effect.on }}</TableCell>
                            <TableCell>
                              <Badge class="border-2 border-black bg-white text-black">
                                {{ effect.effect_type }}
                              </Badge>
                            </TableCell>
                            <TableCell class="font-mono text-xs">
                              {{ effect.capabilities.join(', ') }}
                            </TableCell>
                          </TableRow>
                        </TableBody>
                      </Table>
                    </CardContent>
                  </Card>
                </TabsContent>

                <TabsContent value="source">
                  <Card class="border-4 border-black bg-white shadow-[6px_6px_0_#000]">
                    <CardHeader>
                      <CardTitle class="flex items-center gap-2 text-lg font-black">
                        <Code2 class="size-5" />
                        Workflow source
                      </CardTitle>
                      <CardDescription>
                        Read-only YAML loaded from {{ selectedPath }}.
                      </CardDescription>
                    </CardHeader>
                    <CardContent>
                      <Textarea
                        :model-value="workflowSource"
                        readonly
                        spellcheck="false"
                        aria-label="Workflow YAML source"
                        class="min-h-[520px] resize-none border-2 border-black bg-neutral-950 p-4 font-mono text-xs leading-5 text-lime-200"
                      />
                    </CardContent>
                  </Card>
                </TabsContent>

                <TabsContent value="trace">
                  <Card class="border-4 border-black bg-white shadow-[6px_6px_0_#000]">
                    <CardHeader>
                      <CardTitle class="flex items-center gap-2 text-lg font-black">
                        <GitBranch class="size-5" />
                        Last run trace
                      </CardTitle>
                      <CardDescription>
                        {{ runReport ? `${runReport.run_id} · ${runReport.status}` : 'Run a workflow to populate the trace.' }}
                      </CardDescription>
                    </CardHeader>
                    <CardContent>
                      <ScrollArea class="h-[520px] pr-4">
                        <div
                          v-if="!runReport"
                          class="grid h-80 place-items-center border-2 border-dashed border-black bg-[#f7f0d8] text-center"
                        >
                          <div>
                            <Terminal class="mx-auto mb-3 size-8" />
                            <p class="font-black">No trace loaded</p>
                            <p class="text-sm text-neutral-700">The run button calls the Tauri backend.</p>
                          </div>
                        </div>

                        <ol v-else class="space-y-3">
                          <li
                            v-for="entry in traceRows"
                            :key="entry.id"
                            class="grid grid-cols-[42px_minmax(0,1fr)] gap-3 border-2 border-black bg-[#fffdf4] p-3 shadow-[3px_3px_0_#000]"
                          >
                            <span class="grid size-8 place-items-center border-2 border-black bg-[#d9f99d] font-mono text-xs font-black">
                              {{ entry.sequence }}
                            </span>
                            <div class="min-w-0">
                              <p class="truncate font-black">{{ traceLabel(entry) }}</p>
                              <p class="truncate font-mono text-xs text-neutral-700">
                                {{ traceDetail(entry) }}
                              </p>
                            </div>
                          </li>
                        </ol>
                      </ScrollArea>
                    </CardContent>
                  </Card>
                </TabsContent>

                <TabsContent value="raw">
                  <Card class="border-4 border-black bg-white shadow-[6px_6px_0_#000]">
                    <CardHeader>
                      <CardTitle class="flex items-center gap-2 text-lg font-black">
                        <FileText class="size-5" />
                        Engine payload
                      </CardTitle>
                      <CardDescription>
                        Raw plan/report JSON for building the next inspector views.
                      </CardDescription>
                    </CardHeader>
                    <CardContent>
                      <pre class="max-h-[560px] overflow-auto border-2 border-black bg-neutral-950 p-4 text-xs leading-5 text-lime-200"><code>{{ JSON.stringify(runReport ?? selectedPlan, null, 2) }}</code></pre>
                    </CardContent>
                  </Card>
                </TabsContent>
              </Tabs>
            </div>

            <div class="space-y-4">
              <Card class="border-4 border-black bg-[#d9f99d] shadow-[6px_6px_0_#000]">
                <CardHeader>
                  <CardTitle class="text-lg font-black">Run status</CardTitle>
                  <CardDescription class="text-neutral-800">
                    Current engine response from Tauri.
                  </CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                  <div class="flex items-center gap-3">
                    <div class="grid size-10 place-items-center border-2 border-black bg-white">
                      <CheckCircle2 v-if="runReport?.status === 'succeeded'" class="size-5" />
                      <AlertCircle v-else class="size-5" />
                    </div>
                    <div>
                      <p class="font-black uppercase">
                        {{ runReport?.status ?? (running ? 'running' : 'idle') }}
                      </p>
                      <p class="font-mono text-xs text-neutral-700">
                        {{ runReport?.run_id ?? 'no run id yet' }}
                      </p>
                    </div>
                  </div>
                  <p
                    v-if="runReport?.failure"
                    class="border-2 border-black bg-[#ffde59] p-2 text-sm font-semibold"
                  >
                    {{ runReport.failure.message }}
                  </p>
                </CardContent>
              </Card>

              <Card class="border-4 border-black bg-white shadow-[6px_6px_0_#000]">
                <CardHeader>
                  <CardTitle class="text-lg font-black">Mutating surface</CardTitle>
                  <CardDescription>
                    Write-capable actions called out before execution.
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  <div class="space-y-2">
                    <div
                      v-for="capability in mutatingCapabilities"
                      :key="capability.id"
                      class="border-2 border-black bg-[#ffde59] p-3"
                    >
                      <p class="font-mono text-xs font-black">{{ capability.id }}</p>
                      <p class="text-sm text-neutral-800">{{ capability.description }}</p>
                    </div>
                    <p v-if="mutatingCapabilities.length === 0" class="text-sm text-neutral-700">
                      No write-capable effects in this plan.
                    </p>
                  </div>
                </CardContent>
              </Card>
            </div>
          </div>
        </section>
      </div>
    </main>
  </TooltipProvider>
</template>
