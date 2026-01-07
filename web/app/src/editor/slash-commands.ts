/**
 * Slash command menu for the editor.
 *
 * Provides a command palette that appears when typing "/" at the start
 * of a line or after whitespace, allowing quick insertion of markdown.
 */

import {
	autocompletion,
	type Completion,
	type CompletionContext,
	type CompletionResult,
	snippetCompletion,
} from "@codemirror/autocomplete";

interface SlashCommand {
	label: string;
	description: string;
	snippet: string;
}

const commands: SlashCommand[] = [
	{ label: "Heading 1", description: "Large heading", snippet: "# ${}" },
	{ label: "Heading 2", description: "Medium heading", snippet: "## ${}" },
	{ label: "Heading 3", description: "Small heading", snippet: "### ${}" },
	{ label: "Bold", description: "Bold text", snippet: "**${text}**" },
	{ label: "Italic", description: "Italic text", snippet: "*${text}*" },
	{ label: "Bullet list", description: "Unordered list item", snippet: "- ${}" },
	{
		label: "Numbered list",
		description: "Ordered list item",
		snippet: "1. ${}",
	},
	{ label: "Checkbox", description: "Task list item", snippet: "- [ ] ${}" },
	{
		label: "Code block",
		description: "Fenced code block",
		snippet: "```${lang}\n${}\n```",
	},
	{ label: "Quote", description: "Block quote", snippet: "> ${}" },
	{ label: "Divider", description: "Horizontal rule", snippet: "---\n${}" },
	{ label: "Link", description: "Hyperlink", snippet: "[${title}](${url})" },
	{
		label: "Image",
		description: "Upload image attachment",
		snippet: "![${alt text}](attachment:pending)",
	},
];

function slashCommandSource(
	context: CompletionContext,
): CompletionResult | null {
	// Match "/" at line start or after whitespace
	const match = context.matchBefore(/(?:^|[\s])\/[\w]*$/);
	if (!match) return null;

	// Find where the "/" starts
	const slashPos = match.text.lastIndexOf("/");
	const from = match.from + slashPos;

	// Build completions using snippetCompletion for proper cursor placement
	const options: Completion[] = commands.map((cmd) =>
		snippetCompletion(cmd.snippet, {
			label: "/" + cmd.label,
			detail: cmd.description,
			type: "keyword",
		}),
	);

	return {
		from,
		options,
	};
}

export const slashCommands = autocompletion({
	override: [slashCommandSource],
	icons: false,
	activateOnTyping: true,
	tooltipClass: () => "slash-command-tooltip",
});
