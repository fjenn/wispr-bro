// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Fork UI primitives (S-13; rebuilt in Design Phase 0 on the v2 tokens in
 * `src/fork/styles/tokens.css` and the motion presets in `./motion`).
 * Radix-based where behaviour matters (menus, popovers, tabs, selects,
 * tooltips). Every primitive in every state is on the debug gallery
 * (`#/fork/gallery`). Primitives are frozen for the Phase 1 area agents: a
 * missing variant is a small change here, never a local fork of a primitive.
 * Glyphs come from `./icons`.
 */
export { Badge, type BadgeTone } from "./Badge";
export {
  Banner,
  Hero,
  type BannerPair,
  type BannerProps,
  type BannerSuggestion,
  type HeroProps,
} from "./Banner";
export {
  Button,
  buttonClass,
  FOCUS_RING,
  IconButton,
  type ButtonProps,
  type ButtonSize,
  type ButtonVariant,
  type IconButtonProps,
} from "./Button";
export {
  Card,
  ListContainer,
  ListRow,
  SectionLabel,
  type CardProps,
  type CardTone,
  type ListRowProps,
} from "./Card";
export { Checkbox, type CheckboxProps } from "./Checkbox";
export {
  Chip,
  ChipGroup,
  type ChipGroupProps,
  type ChipProps,
  type ChipSize,
  type RemovableChipProps,
  type ToggleChipProps,
} from "./Chip";
export { Dialog, Sheet, type DialogProps, type SheetProps } from "./Dialog";
export { EmptyState, type EmptyStateProps } from "./EmptyState";
export { Kbd, KbdCombo } from "./Kbd";
export { Menu, type MenuGroup, type MenuItem, type MenuProps } from "./Menu";
export { Popover, type PopoverProps } from "./Popover";
export {
  ProgressDashes,
  ProgressLine,
  Stepper,
  type StepperStage,
} from "./Progress";
export {
  CheckCard,
  OptionCard,
  Radio,
  RadioGroup,
  RadioMark,
  type CheckCardProps,
  type OptionCardProps,
  type RadioGroupProps,
  type RadioProps,
} from "./Radio";
export {
  SegmentedControl,
  type SegmentOption,
  type SegmentedControlProps,
} from "./SegmentedControl";
export {
  Select,
  type SelectGroup,
  type SelectOption,
  type SelectProps,
} from "./Select";
export {
  SettingGroup,
  SettingRow,
  type SettingControlIds,
  type SettingRowProps,
} from "./SettingRow";
export {
  SearchToggle,
  SortMenu,
  type SearchToggleProps,
  type SortMenuProps,
} from "./SearchToggle";
export { Switch, type SwitchProps } from "./Switch";
export { Tabs, TabPanel, type TabItem, type TabsProps } from "./Tabs";
export {
  Tag,
  markNewTagSeen,
  useNewTagVisible,
  type TagKind,
  type TagProps,
} from "./Tag";
export {
  SearchField,
  TextArea,
  TextField,
  type TextAreaProps,
  type TextFieldProps,
} from "./TextField";
export { Tooltip, TooltipProvider, type TooltipProps } from "./Tooltip";
export {
  ThemeScope,
  usePinnedTheme,
  usePinnedThemeProps,
  useWindowTheme,
  type PinnedTheme,
  type ThemeScopeProps,
} from "./ThemeScope";
export { toast, ToastView, type ToastTone, type ToastViewProps } from "./Toast";
export { BrandGlyph, Wordmark, type WordmarkProps } from "./Wordmark";
export { PageHeader } from "./PageHeader";
export { StatNumber } from "./StatNumber";
export * from "./motion";
