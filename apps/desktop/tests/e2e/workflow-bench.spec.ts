import { expect, test, type Page } from '@playwright/test'


declare global {
  interface Window {
    __TAURI_INTERNALS__: {
      invoke: (cmd: string, args: unknown) => Promise<unknown>
      transformCallback: () => number
      unregisterCallback: () => undefined
      convertFileSrc: (filePath: string) => string
    }
    __DEVKNIFE_TAURI_CALLS__: Array<{ cmd: string; args: unknown }>
  }
}
const workflowPath = 'examples/workflows/customer-onboarding-demo.workflow.yaml'
const environmentPath = 'examples/environments/local.yaml'

const workflowSource = `version: devknife.workflow/v1alpha1
name: customer-onboarding-demo
seed_events:
  - id: seed-customer-onboarding
    type: customer.onboarding.requested
handlers: []
`

const runReport = {
  run_id: 'run-demo-001',
  workflow_name: 'customer-onboarding-demo',
  status: 'succeeded',
  failure: null,
  trace: [
    {
      id: 'trace-1',
      sequence: 1,
      kind: {
        type: 'event_seeded',
        event: { event_type: 'customer.onboarding.requested' },
      },
    },
    {
      id: 'trace-2',
      sequence: 2,
      kind: {
        type: 'effect_executed',
        effect: { type: 'rest' },
        observation: { type: 'rest_response' },
      },
    },
    {
      id: 'trace-3',
      sequence: 3,
      kind: {
        type: 'effect_executed',
        effect: { type: 'sqs_send' },
        observation: { type: 'sqs_send' },
      },
    },
  ],
}

async function installTauriMock(page: Page) {
  await page.addInitScript(
    ({ workflowPath, environmentPath, workflowSource, runReport }) => {
      const plan = {
        workflow_name: 'customer-onboarding-demo',
        workflow_version: 'devknife.workflow/v1alpha1',
        required_capabilities: [
          {
            id: 'network.rest',
            risk: 'read',
            description: 'Call REST services',
          },
          {
            id: 'queue.sqs.write',
            risk: 'write',
            description: 'Send messages to SQS queues',
          },
        ],
        effects: [
          {
            handler_index: 0,
            effect_index: 0,
            on: 'customer.onboarding.requested',
            effect_type: 'rest',
            capabilities: ['network.rest'],
          },
          {
            handler_index: 1,
            effect_index: 0,
            on: 'customer.onboarding.event.received',
            effect_type: 'sqs_send',
            capabilities: ['queue.sqs.write'],
          },
        ],
      }

      const calls: Array<{ cmd: string; args: unknown }> = []

      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args: unknown) => {
          calls.push({ cmd, args })
          switch (cmd) {
            case 'list_workflows':
              return [
                {
                  name: 'customer-onboarding-demo',
                  version: 'devknife.workflow/v1alpha1',
                  path: workflowPath,
                  valid: true,
                  validationError: null,
                  seedEventCount: 1,
                  handlerCount: 8,
                  effectCount: 9,
                  capabilityCount: 2,
                },
              ]
            case 'list_environments':
              return [
                {
                  name: 'local',
                  path: environmentPath,
                  valid: true,
                  validationError: null,
                  serviceCount: 4,
                  valueCount: 7,
                  secretCount: 2,
                },
              ]
            case 'plan_workflow_file':
              return plan
            case 'read_workflow_source':
              return workflowSource
            case 'validate_workflow_source':
              return {
                valid: true,
                kind: null,
                message: 'Workflow source is valid.',
                line: null,
                column: null,
              }
            case 'save_workflow_source':
              return null
            case 'list_run_reports':
              return { reports: [], warnings: [] }
            case 'run_workflow_file':
              return runReport
            case 'read_run_report':
              return runReport
            default:
              throw new Error(`Unhandled Tauri command: ${cmd}`)
          }
        },
        transformCallback: () => 1,
        unregisterCallback: () => undefined,
        convertFileSrc: (filePath: string) => filePath,
      }
      window.__DEVKNIFE_TAURI_CALLS__ = calls
    },
    { workflowPath, environmentPath, workflowSource, runReport },
  )
}

test.beforeEach(async ({ page }) => {
  await installTauriMock(page)
})

test('renders workflows, environment bindings, and plan data', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByText('devknife', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: /customer-onboarding-demo/ })).toBeVisible()
  await expect(page.getByText('customer.onboarding.requested')).toBeVisible()
  await expect(page.getByRole('cell', { name: 'queue.sqs.write' })).toBeVisible()
  await expect(page.getByLabel('Runtime environment')).toHaveValue(environmentPath)
  await expect(page.getByText('4 services')).toBeVisible()
})

test('validates the loaded workflow source', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByRole('button', { name: /customer-onboarding-demo/ })).toBeVisible()
  await page.getByRole('tab', { name: 'Source' }).click()
  await expect(page.getByLabel('Workflow YAML source')).toHaveValue(/customer-onboarding-demo/)

  await page.getByRole('button', { name: /Validate/ }).click()
  await expect(page.getByText('Workflow source is valid.')).toBeVisible()
})

test('runs a write-capable workflow after confirmation and filters the trace', async ({ page }) => {
  page.on('dialog', async (dialog) => {
    expect(dialog.message()).toContain('queue.sqs.write')
    await dialog.accept()
  })

  await page.goto('/')
  await page.getByRole('button', { name: 'Run' }).click()

  await expect(page.getByText('run-demo-001 · succeeded')).toBeVisible()
  await expect(page.getByText('effect rest')).toBeVisible()
  await expect(page.getByText('effect sqs_send')).toBeVisible()

  await page.getByLabel('Filter trace entries').fill('sqs')
  await expect(page.getByText('1 / 3')).toBeVisible()
  await expect(page.getByText('effect sqs_send')).toBeVisible()
  await expect(page.getByText('effect rest')).not.toBeVisible()
})
