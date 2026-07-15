export { LABEL_COLORS, labelColor } from '@/entities/case-label/model/label'
export type { AppliedLabel, CustomLabel } from '@/entities/case-label/model/label'
export {
  applyLabel,
  createLabel,
  deleteLabel,
  listAddressLabels,
  listCaseLabels,
  removeLabelFromAddress,
} from '@/entities/case-label/api/labels'
export {
  labelKeys,
  useAddressLabelsQuery,
  useApplyLabelMutation,
  useCaseLabelsQuery,
  useCreateLabelMutation,
  useDeleteLabelMutation,
  useRemoveAddressLabelMutation,
} from '@/entities/case-label/api/queries'
