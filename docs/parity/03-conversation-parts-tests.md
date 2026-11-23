# Component 3: every Fleet test case, and where it went

Written by piece T of `docs/parity/03-conversation-parts.md`, against weave-fleet `main` (the commit that file names).
One row per `it(...)` in the Fleet test files that test the conversation parts: the Rust test that covers it, or the
component that owns it, or why it has no test here. 400 cases are ported (59 of them added by this sweep), 54 belong to
other components, 23 are DOM-only or have no step of their own in the Rust design, 2 do not apply, and 8 are open
in this component (the `@` session chip, the `$` skill chip and file links in running text: Rune's `Markdown` has no
inline-chip hook, see `docs/RUNE.md`).

Paths are repo-relative.
Kinds: `belongs to component N` = another component owns it; `DOM only` = a browser mechanism with no Rune counterpart; `N/A by design` = the Rust architecture has no such step; `open in component 3` = this component's, not yet drawn.


## components/session/__tests__/ActivityStream.background-work.test.ts

| Fleet case | Rust test or owner |
|---|---|
| says stopped, not error, for a shell Fleet stopped | src/tests.rs::activity_stream_and_work_fleet_stopped_says_stopped_not_error_for_a_shell_fleet_stopped |
| asks Fleet for older work when the stream no longer has it | src/tests.rs::activity_stream_and_work_fleet_stopped_asks_fleet_for_older_work_when_the_stream_no_longer_has_it |
| keeps a real failure a failure | src/tests.rs::activity_stream_and_work_fleet_stopped_keeps_a_real_failure_a_failure |

## components/session/__tests__/ActivityStream.commands.test.ts

| Fleet case | Rust test or owner |
|---|---|
| focus-prompt focuses the composer when it names this session | belongs to component 5: composer focus (no session-command bus for it in Rust yet) |
| copy-session-id puts this session's id on the clipboard | belongs to component 8: session actions (copy session id) |
| export-conversation opens the Markdown export dialog for this session only | src/fleet/tests/export.rs::the_palette_command_opens_the_dialog_for_the_open_session_and_esc_closes_it |
| export-raw-messages downloads the messages as JSON named after the session | belongs to component 8: session actions (Export raw messages as JSON) |
| scroll-top scrolls the stream to its top, smoothly, for this session only | src/fleet/tests/scroll.rs::scrolls_to_the_top_and_the_bottom_on_the_session_commands |
| scroll-bottom jumps the stream to its end, for this session only | src/fleet/tests/scroll.rs::scrolls_to_the_top_and_the_bottom_on_the_session_commands |
| show-message scrolls to a message by id, or by the tool call it holds, and marks it | src/fleet/tests/scroll.rs::show_message_finds_a_message_by_the_tool_call_it_holds_and_ignores_an_unknown_one + show_message_brings_a_message_to_the_middle_rings_it_for_1600_ms_and_stops_following_the_end |
| stops answering once it is unmounted | DOM only: unmount removes a window listener; Rune's commands are State methods with nothing to unsubscribe |

## components/session/__tests__/ActivityStream.messages.test.ts

| Fleet case | Rust test or owner |
|---|---|
| names the author, the model and the role of each message | src/conv/tests/message_list.rs::names_the_author_the_model_and_the_role_of_each_message |
| clusters a run of messages from one sender and keeps another sender's apart | src/conv/tests/message_list.rs::clusters_a_run_of_messages_from_one_sender_and_keeps_another_senders_apart |
| captions a prompt a mod sent or rewrote, and no other | src/conv/tests/message_list.rs::captions_a_prompt_a_mod_sent_or_rewrote_and_no_other |
| shows a shell command the user ran as its own block, on the user's side | src/conv/tests/message_list.rs::shows_a_shell_command_the_user_ran_as_its_own_block_on_the_users_side |
| puts a prompt that was sent but not echoed yet after the messages, and drops it once the stream has it | src/conv/tests/message_list.rs::puts_a_prompt_that_was_sent_but_not_echoed_yet_after_the_messages_and_drops_it_once_the_stream_has_it |
| points a sub-agent call at the session it started, once there is one | src/tests.rs::a_sub_agent_call_with_a_session_is_one_row_of_its_own_and_one_without_stays_a_tool_row + src/conv/tests/message_list.rs::a_sub_agent_call_that_has_a_session_stays_visible_and_one_without_does_not |
| offers Improve on a call that loaded a built-in skill only | src/tests_skills.rs::leaves_improve_off_a_skill_that_isn_t_fleet_s_or_hasn_t_finished_loading (the finished built-in call has the button; a running one and the user's own have none) |
| keeps tool calls and questions apart, in order | src/conv/tests/message_list.rs::keeps_tool_calls_and_questions_apart_in_order |
| draws a background notice with how the work ended and no bubble text of its own | src/conv/tests/message_list.rs::draws_a_background_notice_with_how_the_work_ended_and_no_bubble_text_of_its_own |
| hides an assistant message with nothing to show, and keeps an empty user one | src/conv/tests/message_list.rs::hides_an_assistant_message_with_nothing_to_show_and_keeps_an_empty_user_one |
| shows thinking alone as a message | src/conv/tests/message_list.rs::shows_thinking_alone_as_a_message |
| leaves thinking alone out when thinking is hidden | src/conv/tests/message_list.rs::leaves_thinking_alone_out_when_thinking_is_hidden |
| shows only what came after the boundary of a side conversation | src/conv/tests/message_list.rs::shows_only_what_came_after_the_boundary_of_a_side_conversation |
| hands an unchanged message's bubble the very same tools and images when others change | DOM only: Vue identity caches; Rune's equivalent is the row memo, src/tests.rs::a_batch_of_deltas_rebuilds_only_the_last_message |
| builds a message's view again when something its own parts read changes | src/tests.rs::the_row_redraws_when_the_delegation_changes_with_no_change_to_the_message |

## components/session/__tests__/ActivityStream.peer-machine.test.ts

| Fleet case | Rust test or owner |
|---|---|
| opens a sender on another machine in place | src/tests_peer.rs::opens_a_sender_on_another_machine_in_place |
| finds a sender with no machine of its own on this session's machine, opened in place | src/tests_peer.rs::finds_a_sender_with_no_machine_of_its_own_on_this_sessions_machine_opened_in_place |

## components/session/__tests__/ActivityStream.rerender.test.ts

| Fleet case | Rust test or owner |
|---|---|
| re-renders only the bubble whose message changed | src/tests.rs::a_batch_of_deltas_rebuilds_only_the_last_message |
| mounts the newest messages first and the older ones a batch a frame after | DOM only: batched virtual mount replaced by Rune virtual_rows (src/tests.rs::only_the_rows_on_screen_are_built) |
| mounts the rest at once when asked to show a message that isn't mounted yet | DOM only: batched virtual mount replaced by Rune virtual_rows (src/tests.rs::only_the_rows_on_screen_are_built) |

## components/session/__tests__/ActivityStream.scroll.test.ts

| Fleet case | Rust test or owner |
|---|---|
| starts at the bottom, without the jump button | src/fleet/tests/scroll.rs::starts_at_the_bottom_without_the_jump_button |
| offers Jump to latest once scrolled up past the threshold, and scrolls to the bottom when clicked | src/fleet/tests/scroll.rs::offers_jump_to_latest_once_scrolled_up_past_the_threshold_and_scrolls_to_the_bottom_when_clicked |
| keeps the view pinned to the bottom as content grows | src/fleet/tests/scroll.rs::keeps_the_view_pinned_to_the_bottom_as_content_grows |
| leaves the view where it is when scrolled up and content grows | src/conv/tests/pin.rs::leaves_the_view_where_it_is_when_scrolled_up_and_content_grows |
| a small move up that is not the user's (layout shift, rounding) leaves the view pinned | src/conv/tests/pin.rs::a_small_move_up_that_is_not_the_users_layout_shift_rounding_leaves_the_view_pinned |
| a small scroll up by wheel stays put while text streams in | src/fleet/tests/scroll.rs::a_small_scroll_up_by_wheel_stays_put_while_text_streams_in |
| a wheel up inside a code block scrolling on its own is not leaving the bottom | src/conv/tests/pin.rs::a_wheel_up_inside_a_code_block_scrolling_on_its_own_is_not_leaving_the_bottom |
| sending a prompt pins again after a nudge up, so the reply is followed | src/fleet/tests/scroll.rs::sending_a_prompt_pins_again_after_a_nudge_up_so_the_reply_is_followed |
| pins again for a session switch | src/fleet/tests/scroll.rs::pins_again_for_a_session_switch |
| shows the newest 20 first, then mounts older ones a batch a frame | DOM only: batched virtual mount / observers replaced by Rune virtual_rows (src/tests.rs::only_the_rows_on_screen_are_built) |
| mounts every message at once on reaching the top | DOM only: batched virtual mount / observers replaced by Rune virtual_rows (src/tests.rs::only_the_rows_on_screen_are_built) |
| mounts one message at a time as it arrives | DOM only: batched virtual mount / observers replaced by Rune virtual_rows (src/tests.rs::only_the_rows_on_screen_are_built) |
| asks for them when scrolled near the top, and not before | src/fleet/tests/scroll.rs::asks_for_older_messages_when_scrolled_near_the_top_and_not_before |
| does not ask again while they are loading, or when there are none | src/fleet/tests/scroll.rs::does_not_ask_again_while_they_are_loading_or_when_there_are_none |
| keeps the reading position when they arrive above | src/fleet/tests/scroll.rs::keeps_the_reading_position_when_they_arrive_above |
| scrolls to it, highlights it for a moment and keeps every message laid out meanwhile | src/fleet/tests/scroll.rs::show_message_brings_a_message_to_the_middle_rings_it_for_1600_ms_and_stops_following_the_end (the laid-out layout is DOM only) |
| ends the laid-out layout when the scroll ends | DOM only: batched virtual mount / observers replaced by Rune virtual_rows (src/tests.rs::only_the_rows_on_screen_are_built) |
| stops pinning to the bottom once it has scrolled to a message | src/conv/tests/pin.rs::stops_pinning_to_the_bottom_once_it_has_scrolled_to_a_message |
| mounts the unmounted older messages first when the target is among them | DOM only: batched virtual mount / observers replaced by Rune virtual_rows (src/tests.rs::only_the_rows_on_screen_are_built) |
| scrolls to the top and the bottom on the session commands | src/fleet/tests/scroll.rs::scrolls_to_the_top_and_the_bottom_on_the_session_commands |
| disconnects its observers and cancels its work on unmount | DOM only: batched virtual mount / observers replaced by Rune virtual_rows (src/tests.rs::only_the_rows_on_screen_are_built) |

## components/session/__tests__/ActivityStream.thinking.test.ts

| Fleet case | Rust test or owner |
|---|---|
| shows a message that so far is only thinking, marked live while the turn runs | src/conv/tests/message_list.rs::shows_a_message_that_so_far_is_only_thinking_marked_live_while_the_turn_runs |
| isn't live once the turn has ended, or once something follows the thinking | src/conv/tests/message_list.rs::isnt_live_once_the_turn_has_ended_or_once_something_follows_the_thinking |
| leaves out a thinking-only message when thinking is hidden | src/conv/tests/message_list.rs::leaves_thinking_alone_out_when_thinking_is_hidden |

## components/session/__tests__/AgentTaskRow.test.ts

| Fleet case | Rust test or owner |
|---|---|
| names the agent and its task | src/conv/tests/background.rs::agent_task_row_names_the_agent_and_its_task |
| (it.each) it.each([ | src/conv/tests/background.rs::agent_task_row_shows_a_child_as_its_status_word |
| says a child stopped on a question needs input, and opens it to answer | src/conv/tests/background.rs::agent_task_row_says_a_child_stopped_on_a_question_needs_input_and_opens_it_to_answer |
| says a child in the background is running there, not working for the session | src/conv/tests/background.rs::agent_task_row_says_a_child_in_the_background_is_running_there_not_working_for_the_session |
| says a child in the background that asks needs input | src/conv/tests/background.rs::agent_task_row_says_a_child_in_the_background_that_asks_needs_input |
| says a child that came back from the background is done | src/conv/tests/background.rs::agent_task_row_says_a_child_that_came_back_from_the_background_is_done |
| ignores a stale question once the child has finished | src/conv/tests/background.rs::agent_task_row_ignores_a_stale_question_once_the_child_has_finished |
| opens the child session in place | src/tests.rs::every_state_of_the_row_draws_and_a_press_opens_the_child |
| leaves a modified click to the browser | DOM only: a modified click on an anchor's href goes to the browser; Rune has no anchors |
| reads OpenCode's task input | src/conv/tests/background.rs::sub_agent_call_details_read_opencodes_task_input |
| reads OpenCode 2's subagent input | src/conv/tests/background.rs::sub_agent_call_details_read_opencode_2s_subagent_input |
| is empty when the call doesn't say | src/conv/tests/background.rs::sub_agent_call_details_are_empty_when_the_call_doesnt_say |

## components/session/__tests__/BackgroundStrip.test.ts

| Fleet case | Rust test or owner |
|---|---|
| shows nothing when nothing runs | src/conv/tests/work_clock.rs::shows_nothing_when_nothing_runs |
| lists running work with kind, what it is, elapsed time, Output and Stop | src/conv/tests/work_clock.rs::lists_running_work_with_kind_what_it_is_and_elapsed_time + src/tests.rs::the_strip_lists_each_kind_and_offers_only_what_the_work_allows |
| offers no Output when the output can't be read, and no Stop when it can't be stopped on its own | src/tests.rs::the_strip_lists_each_kind_and_offers_only_what_the_work_allows |
| shows finished work with its result and no Stop, then drops it | src/tests.rs::finished_work_shows_its_result_and_no_stop_and_then_drops |
| collapses to its header line and remembers it | src/tests.rs::the_header_collapses_the_list_and_keeps_the_summary (remembering across restarts is open: row 4.4) |
| stops an item and shows it stopped | src/tests.rs::a_stopped_item_is_applied_at_once_and_shows_stopped_without_stop |
| says why a stop was refused | src/tests.rs::a_refused_stop_says_why_under_the_row_and_the_next_press_clears_it |
| opens the tail of the output under the row | src/tests.rs::output_opens_under_its_row_says_it_is_reading_and_closes_again |
| opens a subagent's session | src/tests.rs::open_shows_the_child_session_and_remembers_the_parent_but_only_one_the_list_holds |
| jumps to a monitor's events in the conversation | src/tests.rs::events_press_is_taken_by_a_monitor_with_a_call |
| gives a subagent with no session of its own Details instead of Open, and says what it is doing | src/tests.rs::details_open_and_close_and_say_what_the_agent_was_asked |
| shows a finished subagent's result once, and offers no Details for one with a session | src/fleet/tests/work.rs::says_how_long_and_how_it_ended_in_a_few_words + src/tests.rs::finished_work_shows_its_result_and_no_stop_and_then_drops + src/tests.rs::details_open_and_close_and_say_what_the_agent_was_asked |

## components/session/__tests__/CompactionDivider.test.ts

| Fleet case | Rust test or owner |
|---|---|
| says where the context was compacted, how big it was before and after, and keeps the summary behind a toggle | src/tests.rs::a_compaction_divider_keeps_its_summary_behind_a_toggle + src/conv/tests/compaction.rs::says_how_big_the_context_was_before_and_after_when_the_harness_gives_the_sizes |
| is just the line when the harness gives no sizes or summary | src/conv/tests/compaction.rs::says_how_big_the_context_was_before_and_after_when_the_harness_gives_the_sizes (the empty case) |

## components/session/__tests__/InterruptedCard.test.ts

| Fleet case | Rust test or owner |
|---|---|
| says what was working, the last call and the changed files | src/conv/tests/working.rs::says_what_was_working_the_last_call_and_the_changed_files |
| leaves out the call and the files when Fleet doesn't know them | src/conv/tests/working.rs::leaves_out_the_call_and_the_files_when_fleet_doesnt_know_them |
| says the question was lost and offers Ask again | src/conv/tests/working.rs::says_the_question_was_lost_and_offers_ask_again |
| calls the handlers, and disables the buttons while busy | src/tests.rs::both_buttons_wait_while_a_request_is_on_its_way + src/tests.rs::the_interrupted_card_offers_continue_and_dismiss_and_goes_when_dismissed |
| says Fleet is continuing it by itself and hides Continue | src/tests.rs::a_session_fleet_continues_by_itself_hides_continue |

## components/session/__tests__/MessageBubble.attached-files.test.ts

| Fleet case | Rust test or owner |
|---|---|
| shows the typed text, a chip for the file and the note under it | src/tests.rs::a_message_with_a_note_shows_a_chip_per_file_and_the_note |
| opens the file, keeping the tab on Ctrl-click | belongs to component 11: files and file viewer (preview tabs; Ctrl-click keeps the tab) |
| leaves the agent's replies alone | src/tests.rs::an_attached_files_note_in_an_agents_reply_is_left_as_text |

## components/session/__tests__/MessageBubble.command.test.ts

| Fleet case | Rust test or owner |
|---|---|
| shows the command, not the prompt it expanded to | src/tests.rs::a_command_shows_as_typed_and_the_prompt_opens_behind_show_prompt + src/conv/tests/references.rs::formats_a_slash_command_as_typed |
| shows the expanded prompt behind Show prompt | src/conv/tests/bubble.rs::offers_the_expanded_prompt_behind_show_prompt |
| offers no prompt when the message is the command itself | src/conv/tests/bubble.rs::offers_no_prompt_when_the_message_is_the_command_itself |
| shows a command without arguments by name | src/conv/tests/bubble.rs::shows_a_command_without_arguments_by_name |
| shows any other message as it is | src/conv/tests/bubble.rs::reads_markdown_only_where_there_is_some (a message without a command is drawn as its text) |

## components/session/__tests__/MessageBubble.file-links.test.ts

| Fleet case | Rust test or owner |
|---|---|
| links the paths that are files in the session's folder, and leaves the rest as code | open in component 3, not drawn yet: file links in running text need an inline-chip hook Rune's Markdown lacks (docs/RUNE.md); opening a tab belongs to component 11 |
| opens a preview tab at the line on a click, and keeps the tab on a Ctrl-click | open in component 3, not drawn yet: file links in running text need an inline-chip hook Rune's Markdown lacks (docs/RUNE.md); opening a tab belongs to component 11 |
| opens from the keyboard with Enter | open in component 3, not drawn yet: file links in running text need an inline-chip hook Rune's Markdown lacks (docs/RUNE.md); opening a tab belongs to component 11 |
| doesn't open when the click ended a drag that selected text | open in component 3, not drawn yet: file links in running text need an inline-chip hook Rune's Markdown lacks (docs/RUNE.md); opening a tab belongs to component 11 |

## components/session/__tests__/MessageBubble.meta-pill.test.ts

| Fleet case | Rust test or owner |
|---|---|

## components/session/__tests__/MessageBubble.session-references.test.ts

| Fleet case | Rust test or owner |
|---|---|
| shows each referenced session as a chip with its title | open in component 3, not drawn yet: `@` session chips in running text need an inline-chip hook Rune's Markdown lacks (docs/RUNE.md); the parse is src/conv/tests/references.rs::reads_text_as_a_file family and src/fleet/tests/references.rs::a_prompt_carries_the_sessions_still_in_the_text_and_the_skills_it_names |
| opens the session when its chip is clicked | open in component 3, not drawn yet (see row above); opening a session is src/tests.rs::clicking_a_session_opens_it |
| leaves a message without references as text | open in component 3, not drawn yet (no chips are drawn, so every message is text) |

## components/session/__tests__/MessageBubble.skills.test.ts

| Fleet case | Rust test or owner |
|---|---|
| shows each skill in your message as a chip, the one Claude Code got as a note in amber | open in component 3, not drawn yet: `$` skill chips in running text (docs/RUNE.md) |
| leaves unknown names, code and the agent's replies as text | src/composer/tests_autocomplete.rs::skill_names_in_leaves_unknown_names_longer_tokens_and_code_alone (the finder; the chips are not drawn yet) |

## components/session/__tests__/MessageBubble.tool-rows.test.ts

| Fleet case | Rust test or owner |
|---|---|
| draws an ordinary call as a ToolCard and nothing else | src/tests.rs::a_sub_agent_call_with_a_session_is_one_row_of_its_own_and_one_without_stays_a_tool_row (the `tool-` row) |
| draws an unknown call as a ToolCard too | src/conv/tests/registry.rs::falls_back_for_a_tool_it_does_not_know; every kind is drawn by one `tool_row` |
| draws a call with a delegation as an AgentTaskRow, with no ToolCard | src/tests.rs::a_sub_agent_call_with_a_session_is_one_row_of_its_own_and_one_without_stays_a_tool_row |
| draws a sub-agent call that has no delegation yet as a plain ToolCard | src/tests.rs::a_sub_agent_call_with_a_session_is_one_row_of_its_own_and_one_without_stays_a_tool_row |
| passes the call's fields to the ToolCard | DOM only: pins Vue props through stubs; the Rune row reads the part directly (src/conv/tests/card.rs::keeps_the_fields_the_card_shows) |
| draws one row per call, in order, mixing the kinds | DOM only: pins which Vue component is chosen, through stubs; one `tool_row` per call in Rune, order by src/conv/tests/message_list.rs::keeps_tool_calls_and_questions_apart_in_order |
| draws nothing for a message with no calls | src/conv/tests/tool_row.rs::draws_nothing_without_calls |
| adds a ToolScreenshot after the ToolCard when the call kept a screenshot | src/tests_images.rs::shows_a_thumbnail_under_the_call_even_with_the_calls_row_folded |
| draws no screenshot for a delegation, even if the call carries one | by construction: a delegation is drawn by `agent_row`, which has no thumbnail (no separate test) |
| keys the screenshot on the item, not the tool kind: any call with one gets a thumbnail | src/conv/tests/message_list.rs::reads_a_screenshot_a_page_and_a_canvas_off_the_calls_metadata (any kind) |
| adds BrowserSteps under a %s call | src/conv/tests/sweep.rs::message_bubble_tool_rows_browser_steps::adds_browser_steps_under_the_calls_that_drive_the_browser_and_no_other_in_any_case + src/tests_browser.rs::a_call_that_used_the_browser_lists_its_steps_in_order |
| says the steps are no longer running once the call is not Running | src/tests_browser.rs::the_head_says_used_and_how_many_once_the_call_is_done + src/conv/tests/browser.rs::the_head_counts_steps_and_says_singular_only_for_one |
| (it.each) it.each(["fleet_browser_open", "fleet_browser_screenshot", "fleet_app_start", "bash", "read", "task"])( | src/conv/tests/sweep.rs::message_bubble_tool_rows_browser_steps::adds_browser_steps_under_the_calls_that_drive_the_browser_and_no_other_in_any_case |
| adds no BrowserSteps without a session id | DOM only: a Rune conversation always belongs to the open session |
| adds no BrowserSteps to a delegation | by construction: a delegation is drawn by `agent_row`, which has no steps (no separate test) |
| matches the kind in any case: Execute gets them too | src/conv/tests/sweep.rs::message_bubble_tool_rows_browser_steps::adds_browser_steps_under_the_calls_that_drive_the_browser_and_no_other_in_any_case |
| draws a call's page outside the .msg-tools box, after it | belongs to component 14: web content (pages in the conversation, `ConversationPage`) |
| draws one page per call that has one, in call order | belongs to component 14: web content (pages in the conversation, `ConversationPage`) |
| draws no page for a delegation | belongs to component 14: web content (pages in the conversation, `ConversationPage`) |
| draws a page for any call that has one, whatever its kind | belongs to component 14: web content (pages in the conversation, `ConversationPage`) |
| has no paging of its own: every call is drawn, however many | src/conv/tests/sweep.rs::message_bubble_tool_rows_pages::has_no_paging_of_its_own_every_call_is_drawn_however_many |
| collapses reads that are still streaming too, with a spinner on the row, even before their path arrives | src/conv/tests/tool_row.rs::takes_reads_still_streaming_into_the_row_whether_or_not_their_path_has_arrived |
| collapses completed reads in a row into one Read N files row that opens to the list | src/tests.rs::clicking_the_reads_row_opens_and_closes_the_folded_reads + src/conv/tests/tool_row.rs::collapses_two_or_more_completed_reads_in_a_row |

## components/session/__tests__/MessageToolList.test.ts

| Fleet case | Rust test or owner |
|---|---|
| draws nothing without calls | src/conv/tests/tool_row.rs::draws_nothing_without_calls |
| draws a ToolCard per call inside the box | DOM only: pins Vue components through stubs; see MessageBubble.tool-rows rows |
| draws a delegated call as an AgentTaskRow | src/tests.rs::a_sub_agent_call_with_a_session_is_one_row_of_its_own_and_one_without_stays_a_tool_row |
| adds browser steps to a Code Mode call, only with a session | src/conv/tests/sweep.rs::message_bubble_tool_rows_browser_steps::adds_browser_steps_under_the_calls_that_drive_the_browser_and_no_other_in_any_case |
| draws a call's page after the box, not inside it | belongs to component 14: web content (`ConversationPage`) |
| passes a row's events up with the call's title and id | DOM only: Vue emits; the `show-canvas` event belongs to component 10 (canvas Show) and `improve` to component 9 (skills) |

## components/session/__tests__/ReasoningBlock.test.ts

| Fleet case | Rust test or owner |
|---|---|
| is folded to one line by default and opens on click | src/tests.rs::reasoning_is_folded_to_one_line_by_default_and_opens_on_click |
| follows the newest sentence while live | src/tests.rs::reasoning_follows_the_newest_sentence_while_live |
| shows every block in full when set to Open | src/tests.rs::reasoning_shows_every_block_in_full_when_set_to_open_and_nothing_when_hidden |
| shows nothing when set to Hidden, and remembers the choice | src/tests.rs::reasoning_shows_every_block_in_full_when_set_to_open_and_nothing_when_hidden (hidden); remembering the choice belongs to component 16: settings (thinking visibility is not persisted yet) |

## components/session/__tests__/ShellCommandBlock.test.ts

| Fleet case | Rust test or owner |
|---|---|
| shows the command as the user's, with its output | src/tests.rs::a_shell_command_is_the_users_block_with_its_state |
| marks a failed command with its exit code | src/conv/tests/shell.rs::says_how_a_command_ended + src/tests.rs::a_shell_command_is_the_users_block_with_its_state |
| says a running command is running, and that a finished one printed nothing | src/tests.rs::a_shell_command_is_the_users_block_with_its_state + src/conv/tests/shell.rs::says_how_a_command_ended |
| collapses long output to its last lines until asked for the rest | src/conv/tests/shell.rs::collapses_long_output_to_its_last_lines_until_asked_for_the_rest |
| says when the harness kept only part of the output | src/tests.rs::a_shell_commands_long_output_collapses_to_its_last_lines_until_asked (the `truncated` note, scene s6) |

## components/session/__tests__/ToolCard.characterise.test.ts

| Fleet case | Rust test or owner |
|---|---|
| draws ${name} as it always did | belongs to component 22: mods (snapshot characterisation of the mod slot that mods draw into) |
| draws a row per call as it always did | belongs to component 22: mods (snapshot characterisation of the mod slot that mods draw into) |

## components/session/__tests__/ToolCard.mods.test.ts

| Fleet case | Rust test or owner |
|---|---|
| caps the mod slot at 60% and floors the title at min(30%, 12ch) | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| marks the header only when a mod draws, and leaves today's rules for plain titles alone | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| draws the tree in the header and drops the check and the preview | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| keeps Fleet's +N -N after the tree, and drops only the check and the preview | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| keeps Running, Error, Stopped, Background and the buttons | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| a null tree draws nothing but still drops the result and the preview | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| is keyed by session and call id, not by the tool | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| passes its session id to the tree | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| draws Fleet's own for an invalid tree | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| redraws when contributed and when removed, and removal restores Fleet's drawing | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| sends a press to onAction as desktop, and does not toggle the row | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| shows no draft mark on the row | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| replaces the body, keeping the line, the result and the preview | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| draws Fleet's own body where the tree says Fleet | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| a null tree is an empty body, not No output captured | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| draws Fleet's own for an invalid tree, and after removal | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| sends a press to onAction as desktop | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| both sites can draw at once | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| passes the session and call ids to the row | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| contributing one row's view redraws that row only | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| contributing a view for a call that is not on screen redraws nothing | belongs to component 22: mods (a mod's header and body trees on a tool card) |
| removing one row's view redraws that row only | belongs to component 22: mods (a mod's header and body trees on a tool card) |

## components/session/__tests__/ToolCard.rows.test.ts

| Fleet case | Rust test or owner |
|---|---|
| (it.each) it.each([ | src/conv/tests/tool_row.rs::shows_a_running_error_or_background_call_with_a_glyph |
| labels a Background call with the word as well as the glyph | src/conv/tests/tool_row.rs::shows_a_running_error_or_background_call_with_a_glyph (glyph); the word is drawn by view/toolrow.rs `status_word`, not tested |
| shows a Completed call with no tick and no glyph: the turn's fold says it | src/conv/tests/tool_row.rs::shows_a_completed_call_with_no_tick_and_no_glyph |
| shows a Stopped call as the word Stopped, with no glyph and no check | src/conv/tests/tool_row.rs::shows_a_stopped_call_as_the_word_with_no_glyph |
| shows a %s call with no indicator at all | src/conv/tests/tool_row.rs::shows_a_pending_cancelled_or_unknown_call_with_no_indicator_at_all |
| defaults to Completed when no status is given | src/conv/tests/tool_row.rs::defaults_to_pending_when_no_status_is_given_and_capitalises_the_harness_word |
| shows +N and -N instead of the check when a completed call changed a file | src/fleet/tests/cards.rs::tool_card_status_indicator::shows_plus_n_and_minus_n_instead_of_the_check_when_a_completed_call_changed_a_file |
| shows the glyph, not the counts, while an edit is still running | src/fleet/tests/cards.rs::tool_card_status_indicator::shows_the_glyph_not_the_counts_while_an_edit_is_still_running |
| shows the check when the diff has only context lines | src/fleet/tests/cards.rs::tool_card_status_indicator::shows_the_check_when_the_diff_has_only_context_lines |
| starts open unless told it starts collapsed | src/conv/tests/card.rs::starts_every_call_collapsed_except_one_that_failed |
| keeps the header, and no preview line, when collapsed | src/tests.rs::a_call_that_failed_starts_open_with_what_it_printed_and_closes_to_a_preview_line |
| follows the native toggle: opening and closing the details flips it | src/conv/tests/card.rs::follows_the_native_toggle_opening_and_closing_flips_it |
| follows initiallyCollapsed when the prop changes | src/conv/tests/card.rs::follows_initially_collapsed_when_it_changes |
| stays open for a diff when inline diffs are on, even if told to start collapsed | src/conv/tests/card.rs::stays_open_for_a_diff_when_inline_diffs_are_on_even_if_told_to_start_collapsed |
| starts collapsed for a diff when inline diffs are off | src/conv/tests/card.rs::starts_collapsed_for_a_diff_when_inline_diffs_are_off |
| a write opens on what it wrote even with inline diffs off, twelve lines then Show all | src/conv/tests/card.rs::a_write_opens_on_what_it_wrote_even_with_inline_diffs_off + src/tests.rs::a_write_opens_on_its_diff_twelve_lines_then_show_all |
| a write shows +N −N on its line | src/fleet/tests/cards.rs::a_write_shows_plus_n_minus_n_on_its_line |
| a shell row shows how long it took, and its open body has one header and one copy icon | src/conv/tests/card.rs::a_shell_row_shows_how_long_it_took_and_its_open_body_has_one_header |
| reads are dimmed | src/conv/tests/tool_row.rs::reads_are_dimmed |
| a failed call keeps its preview line | src/conv/tests/card.rs::a_failed_call_keeps_its_preview_line |
| (it.each) it.each([ | src/conv/tests/sweep.rs::tool_card_label_and_detail::labels_a_call_by_its_heading |
| shows the title as a quiet detail, or as a pill for glob and grep | src/conv/tests/registry.rs::the_pattern_pill_is_glob_and_grep_in_any_case (the pill rule; the drawing is view/toolrow.rs) |
| uses one icon for a tool it has no icon for, the same one tools sharing a mapping get | src/conv/tests/sweep.rs::tool_card_label_and_detail::uses_one_icon_for_a_tool_it_has_no_icon_for_the_same_one_tools_sharing_a_mapping_get |
| offers Show only on a finished call that has a canvas | belongs to component 10: content panel (canvas Show button; the list only carries the `canvas` flag) |
| keeps the fields the card shows, and the raw input | src/conv/tests/card.rs::keeps_the_fields_the_card_shows |
| carries no input for a call that sent none | DOM only: Rust's `tool_body` reads the raw state; there is no `input` copy to carry |
| starts every call collapsed except one that failed | src/conv/tests/card.rs::starts_every_call_collapsed_except_one_that_failed |
| capitalises the harness status and calls a missing one Pending | src/conv/tests/tool_row.rs::defaults_to_pending_when_no_status_is_given_and_capitalises_the_harness_word |
| marks glob and grep as pattern tools | src/conv/tests/registry.rs::the_pattern_pill_is_glob_and_grep_in_any_case |
| falls back to the tool's name for the title of a tool it can't label | src/conv/tests/registry.rs::falls_back_to_the_tool_name_where_tool_labels_has_no_case |
| takes the title Fleet's answer gave for Fleet's own tools only | src/fleet/tests/cards.rs::shows_a_browser_tools_title_and_address_and_the_canvas_it_opened + src/conv/tests/registry.rs::fleets_titled_tools_take_their_title_from_fleets_answer_the_memory_and_browser_read_act_tools_do_not |
| reads a kept screenshot and page from the call's metadata | src/conv/tests/message_list.rs::reads_a_screenshot_a_page_and_a_canvas_off_the_calls_metadata + src/conv/tests/image_sniff.rs::a_call_keeps_its_screenshot_under_the_session_that_took_it |
| never sets a delegation: ActivityStream adds that afterwards | src/conv/tests/message_list.rs::a_sub_agent_call_that_has_a_session_stays_visible_and_one_without_does_not (only the list context sets it) |

## components/session/__tests__/ToolCard.test.ts

| Fleet case | Rust test or owner |
|---|---|
| offers Improve on a finished call that loaded a built-in skill, without opening the row | src/tests_skills.rs::offers_improve_on_a_finished_call_that_loaded_a_built_in_skill_without_opening_the_row |
| lets a HydraFusion pass row wrap its facts and keeps a path row to one clipped line | DOM only: CSS wrapping of a pass row's facts; the facts are src/conv/tests/registry.rs::reads_as_role_model_and_time_once_the_pass_has_finished |
| leaves Improve off a skill that isn't Fleet's or hasn't finished loading | src/tests_skills.rs::leaves_improve_off_a_skill_that_isn_t_fleet_s_or_hasn_t_finished_loading |
| renders_stable_test_ids_for_summary_output_and_empty_state | src/tests.rs::a_summary_shows_above_the_output + src/tests.rs::a_call_that_printed_nothing_says_so |
| renders_empty_state_with_stable_selector_when_tool_has_no_content | src/tests.rs::a_call_that_printed_nothing_says_so |
| shows_diff_rows_with_stable_selectors_when_inline_diffs_are_enabled | src/tests.rs::a_row_that_is_not_a_write_shows_a_diff_only_with_inline_diffs_on |
| offers to show the canvas a finished tool opened | belongs to component 10: content panel (canvas Show button) | **Closed by component 10 (18 November 2026):** ported (10 pieces C and M).
| says quietly that a retired diagram is gone instead of offering to show it | belongs to component 10: content panel (retired diagram canvases) |

## components/session/__tests__/ToolOutputBody.test.ts

| Fleet case | Rust test or owner |
|---|---|
| draws short output whole, coloured, with its line count | src/conv/tests/output.rs::draws_short_output_whole_coloured_with_its_line_count |
| shows markup in the output as text | src/conv/tests/output.rs::never_turns_the_output_into_markup |
| shows the last 12 lines until asked for the rest, then only the last 12 again | src/conv/tests/output.rs::shows_the_last_12_lines_until_asked_for_the_rest_then_only_the_last_12_again |
| draws at most the last 2,000 lines when expanded, and says so | src/conv/tests/output.rs::draws_at_most_the_last_2000_lines_when_expanded_and_says_so |
| copies the whole output as plain text, with no codes and only the last frame of a progress line | src/conv/tests/output.rs::copies_the_whole_output_as_plain_text_with_no_codes_and_only_the_last_frame_of_a_progress_line |
| stays quiet when the clipboard refuses | DOM only: navigator.clipboard.writeText rejects in a browser; Rune's clipboard write has no failure to swallow |
| draws no body while the row is closed, and the output once it opens | src/tests.rs::a_closed_card_draws_no_body_and_a_click_on_its_row_opens_and_closes_it |

## components/session/__tests__/TurnRetryBar.test.ts

| Fleet case | Rust test or owner |
|---|---|
| says when Fleet tries again, and that it waits for the limit's reset | src/conv/tests/retry.rs::says_when_fleet_tries_again_and_that_it_waits_for_the_limits_reset |
| counts down, by the second in the last minute | src/conv/tests/retry.rs::counts_down_by_the_second_in_the_last_minute |
| says which attempt it is when the limit keeps stopping the session | src/conv/tests/retry.rs::says_which_attempt_it_is_when_the_limit_keeps_stopping_the_session |
| offers Try now and Don't retry | src/tests.rs::a_limit_shows_the_bar_and_hides_try_again_until_it_is_taken_away + src/tests.rs::try_again_sends_the_last_prompt_again |
| shows why an action failed instead of the explanation | src/tests.rs::a_failed_action_says_why_and_a_second_answer_clears_it |

## components/session/__tests__/WorkingIndicator.test.ts

| Fleet case | Rust test or owner |
|---|---|
| shows the working glyph and word | src/conv/tests/working.rs::shows_the_working_glyph_and_word |
| counts up from the prompt | src/conv/tests/working.rs::counts_up_from_the_prompt |
| says it needs input while a question waits | src/conv/tests/working.rs::says_it_needs_input_while_a_question_waits |
| never shows a negative time when clocks disagree | src/conv/tests/working.rs::never_shows_a_negative_time_when_clocks_disagree |
| says it's retrying, which attempt out of how many, when and why, counting down | src/conv/tests/working.rs::says_its_retrying_which_attempt_out_of_how_many_when_and_why_counting_down |
| says so instead of working on: no ticking dots, no growing time | src/conv/tests/working.rs::says_so_instead_of_working_on_no_ticking_dots_no_growing_time |
| adds the call and its own time after the turn's | src/conv/tests/working.rs::adds_the_call_and_its_own_time_after_the_turns |
| says the message waits when it's held | src/conv/tests/working.rs::says_the_message_waits_when_its_held |
| shows only Working with no call | src/conv/tests/working.rs::shows_only_working_with_no_call |

## components/session/__tests__/activity-stream-utils.test.ts

| Fleet case | Rust test or owner |
|---|---|
| shows the unified diff kept on the call's metadata, with its line numbers | src/conv/tests/sweep.rs::to_tool_card_item_diff::shows_the_unified_diff_kept_on_the_calls_metadata_with_its_line_numbers |
| prefers diff lines the harness attached | src/conv/tests/sweep.rs::to_tool_card_item_diff::prefers_diff_lines_the_harness_attached |
| draws a call whose work runs on in the background as Background, whatever the call says | src/conv/tests/sweep.rs::to_tool_card_item::draws_a_call_whose_work_runs_on_in_the_background_as_background_whatever_the_call_says (new `work_status`; NOT yet wired into the card, see report) |
| says Stopped for work Fleet stopped, on every harness | src/conv/tests/sweep.rs::to_tool_card_item::says_stopped_for_work_fleet_stopped_on_every_harness (Claude Code half needs `work_status` wired into the card) |
| shows a browser tool's title and address, and the canvas it opened | src/fleet/tests/cards.rs::shows_a_browser_tools_title_and_address_and_the_canvas_it_opened |
| offers no canvas for a retired diagram call, by any harness's name for it | belongs to component 10: content panel (retired diagram canvases; `ListTool.canvas` does not know them yet) |
| shows a saved note as the note itself, whatever the harness says the call was | src/fleet/tests/cards.rs::shows_a_saved_note_as_the_note_itself_whatever_the_harness_says_the_call_was |
| previews a call's first line of text without its terminal codes | src/conv/tests/card.rs::takes_the_first_line_with_text_once_codes_are_gone_and_counts_lines |
| points a screenshot call at the copy Fleet kept, under the session it names | src/conv/tests/image_sniff.rs::a_call_keeps_its_screenshot_under_the_session_that_took_it |
| gives a call no screenshot when its metadata doesn't name a whole one | src/conv/tests/image_sniff.rs::a_screenshot_missing_any_of_its_four_fields_or_with_no_size_is_none |
| points a page shown in the conversation at Fleet's copy, and gives other calls none | belongs to component 14: web content (pages shown in the conversation, `toolPage`) |
| draws a page shown again in the same turn once, at its last show, and keeps earlier turns' pages | belongs to component 14: web content (`withoutRedrawnPages`) |
| uses_result_as_output_when_output_is_absent | src/conv/tests/sweep.rs::to_tool_card_item::uses_result_as_output_when_output_is_absent |
| derives_title_status_summary_and_collapsed_state | src/conv/tests/sweep.rs::to_tool_card_item::derives_title_status_summary_and_collapsed_state |
| falls_back_to_remaining_state_output_and_normalizes_diff_lines | src/conv/tests/sweep.rs::to_tool_card_item::falls_back_to_remaining_state_output_and_normalizes_diff_lines |
| preserves_falsy_output_values_and_avoids_diff_only_json_fallback | src/conv/tests/sweep.rs::to_tool_card_item::preserves_falsy_output_values_and_avoids_diff_only_json_fallback |
| returns_pending_defaults_when_state_is_not_a_record | src/conv/tests/sweep.rs::to_tool_card_item::returns_pending_defaults_when_state_is_not_a_record |
| links OpenCode's task and OpenCode 2's subagent calls to their child session | src/conv/tests/registry.rs::the_subagent_set_is_task_agent_and_subagent_either_case |

## composables/__tests__/use-agent-browser.test.ts

| Fleet case | Rust test or owner |
|---|---|
| loads the agent's tabs and steps when the session opens | covered with the other loaders by src/fleet/tests/queue.rs::loads_the_queue_when_the_session_opens_and_again_after_a_reconnect (`open_session` and `on_reconnected` call `load_browser`; the request itself is not served by that fake; the agent's tabs belong to component 10) |
| applies browser.step events from the session's topic | src/fleet/tests/composables.rs::use_agent_browser::applies_browser_step_events_from_the_sessions_topic |
| ignores other events and steps for another session | src/fleet/tests/composables.rs::use_agent_browser::ignores_other_events_and_steps_for_another_session |
| loads again after a reconnect | covered with the other loaders by src/fleet/tests/queue.rs::loads_the_queue_when_the_session_opens_and_again_after_a_reconnect (`on_reconnected` calls `load_browser`) |
| follows the session when it changes, and does nothing without one | belongs to component 2: topic subscriptions follow the open session (src/fleet/tests/reconnect.rs::every_topic_is_subscribed_again_after_a_connect) |

## composables/__tests__/use-running-work.test.ts

| Fleet case | Rust test or owner |
|---|---|
| loads the session's work when no stream has said yet | N/A by design: the stream's snapshot carries the work (src/fleet/tests/work.rs::starts_from_the_snapshots_runningwork); only older work is asked for (src/tests.rs::activity_stream_and_work_fleet_stopped_asks_fleet_for_older_work_when_the_stream_no_longer_has_it) |
| takes the stream's work and doesn't load what the stream already has | src/fleet/tests/work.rs::starts_from_the_snapshots_runningwork |
| follows work.* events on the sessions topic | src/fleet/tests/work.rs::adds_work_on_work_started_changes_it_on_work_updated_and_ends_it_on_work_ended |
| keeps a finished item with its result for a while, then drops it, counting from when this browser saw it end | src/conv/tests/work_clock.rs::counts_the_linger_from_when_this_window_saw_the_end |
| doesn't show work that had long ended when the session opened | src/conv/tests/sweep.rs::use_running_work::doesnt_show_work_that_had_long_ended_when_the_session_opened |
| ticks every second in a time's first minute, then once a minute | src/conv/tests/work_clock.rs::ticks_every_second_in_the_first_minute |
| stops an item through Fleet and shows it ended at once | src/tests.rs::a_stopped_item_is_applied_at_once_and_shows_stopped_without_stop + src/tests.rs::stop_and_output_calls_go_to_fleets_item_id_and_read_the_answer_as_the_server_writes_it |
| says why Fleet refused to stop it | src/fleet/tests/composables.rs::use_running_work::says_why_fleet_refused_to_stop_it + src/tests.rs::a_refused_stop_says_why_under_the_row_and_the_next_press_clears_it |
| reads a page of output from an offset | src/conv/tests/work_output.rs::a_short_output_is_read_whole_in_one_page + src/tests.rs::stop_and_output_calls_go_to_fleets_item_id_and_read_the_answer_as_the_server_writes_it |
| loads again after a reconnect | N/A by design: a reconnect brings a new snapshot, which carries the work (src/fleet/tests/work.rs::starts_from_the_snapshots_runningwork) |
| loads what runs everywhere and groups it by session | belongs to component 9: app frame (status bar's running work across sessions) |
| counts work as it starts and stops counting it when it ends, without asking again | belongs to component 9: app frame (status bar's running work across sessions) |

## composables/__tests__/use-session-interruption.test.ts

| Fleet case | Rust test or owner |
|---|---|
| has no interruption on a 204 | src/fleet/tests/composables.rs::has_no_interruption_on_a_204 |
| loads the interruption on a 200 when the session opens | src/fleet/tests/composables.rs::loads_the_interruption_on_a_200_when_the_session_opens |
| follows session.interrupted events, including the one that clears it | src/fleet/tests/composables.rs::follows_session_interrupted_events_including_the_one_that_clears_it |
| ignores another session's event | src/fleet/tests/composables.rs::ignores_another_sessions_event |
| loads again after a reconnect | covered with the other loaders by src/fleet/tests/queue.rs::loads_the_queue_when_the_session_opens_and_again_after_a_reconnect (`on_reconnected` calls `load_interruption`) |
| continues through the endpoint and clears the interruption | src/fleet/tests/composables.rs::continues_or_dismisses_through_the_endpoint_and_clears_the_interruption |
| dismisses through the endpoint and clears the interruption | src/fleet/tests/composables.rs::continues_or_dismisses_through_the_endpoint_and_clears_the_interruption |
| says so when Fleet refuses | src/fleet/tests/composables.rs::says_so_when_fleet_refuses |

## composables/__tests__/use-session-retry.test.ts

| Fleet case | Rust test or owner |
|---|---|
| has no retry when the session has none waiting | src/fleet/tests/composables.rs::has_no_retry_when_the_session_has_none_waiting |
| loads the waiting retry when the session opens | src/fleet/tests/composables.rs::loads_the_waiting_retry_when_the_session_opens |
| follows session.retry events, including the one that takes it away | src/fleet/tests/composables.rs::follows_session_retry_events_including_the_one_that_takes_it_away |
| ignores another session's retry and other events | src/fleet/tests/composables.rs::ignores_another_sessions_retry_and_other_events |
| loads again after a reconnect and stops listening when unmounted | covered with the other loaders by src/fleet/tests/queue.rs::loads_the_queue_when_the_session_opens_and_again_after_a_reconnect (`on_reconnected` calls `load_retry`); stop-listening-on-unmount is DOM only |
| clears the retry once Fleet sends it now or calls it off | src/fleet/tests/composables.rs::clears_the_retry_once_fleet_sends_it_now_or_calls_it_off |
| says so when Fleet refuses, and loads again | src/fleet/tests/composables.rs::says_so_when_fleet_refuses_and_loads_again |

## lib/__tests__/background-work.test.ts

| Fleet case | Rust test or owner |
|---|---|
| reads a finished shell | src/conv/tests/background.rs::parse_background_notice_reads_a_finished_shell |
| reads a finished subagent, and how it ended | src/conv/tests/background.rs::parse_background_notice_reads_a_finished_subagent_and_how_it_ended |
| leaves an ordinary prompt alone | src/conv/tests/background.rs::parse_background_notice_leaves_an_ordinary_prompt_alone |
| says Background while the work runs, although the call itself succeeded | src/conv/tests/background.rs::a_backgrounded_calls_card_says_background_while_the_work_runs_although_the_call_itself_succeeded |
| is finished by the notice that names its shell | src/conv/tests/background.rs::a_backgrounded_calls_card_is_finished_by_the_notice_that_names_its_shell |
| is finished by the notice that names its child session | src/conv/tests/background.rs::a_backgrounded_calls_card_is_finished_by_the_notice_that_names_its_child_session |
| leaves an ordinary call alone | src/conv/tests/background.rs::an_ordinary_call_is_left_alone |
| leaves a subagent that is merely running alone | src/conv/tests/background.rs::a_subagent_that_is_merely_running_is_left_alone |

## lib/__tests__/compaction.test.ts

| Fleet case | Rust test or owner |
|---|---|
| says how big the context was before and after, when the harness gives the sizes | src/conv/tests/compaction.rs::says_how_big_the_context_was_before_and_after_when_the_harness_gives_the_sizes |
| folds a summary message into the divider before it | src/conv/tests/compaction.rs::folds_a_summary_message_into_the_divider_before_it |
| keeps a divider's own summary, and still hides the message | src/conv/tests/compaction.rs::keeps_a_dividers_own_summary_and_still_hides_the_message |
| shows a summary whose divider is on an older page as it is | src/conv/tests/compaction.rs::shows_a_summary_whose_divider_is_on_an_older_page_as_it_is |

## lib/__tests__/conversation-find.test.ts

| Fleet case | Rust test or owner |
|---|---|
| ignores case and doesn't overlap | src/conv/tests/find.rs::count_occurrences_ignores_case_and_doesnt_overlap |
| counts messages only by default, oldest first | src/conv/tests/find.rs::find_hits_counts_messages_only_by_default_oldest_first |
| adds tool output when asked, after the message's own text, and says where it is | src/conv/tests/find.rs::find_hits_adds_tool_output_when_asked_after_the_messages_own_text_and_says_where_it_is |
| finds nothing for an empty query | src/conv/tests/find.rs::find_hits_finds_nothing_for_an_empty_query |
| lets fields, editors, the terminal and dialogs keep the key | src/conv/tests/find.rs::ctrl_f_focus_rules_lets_fields_editors_the_terminal_and_dialogs_keep_the_key |
| matches Ctrl+F, or ⌘F on a Mac, and nothing with extra modifiers | src/conv/tests/find.rs::ctrl_f_focus_rules_matches_ctrl_f_or_cmd_f_on_a_mac_and_nothing_with_extra_modifiers |
| finds a match that runs across inline elements and skips buttons | DOM only: DOM Range over inline elements, skipping buttons; the text rule is src/conv/tests/find.rs::plain_ranges_ignore_case_do_not_overlap_and_stay_on_letters |
| names a turn by its prompt, for the prompt and every agent message after it | src/conv/tests/fold.rs (the `turn_ids_of` case, line ~432) |

## lib/__tests__/diff-parser.test.ts

| Fleet case | Rust test or owner |
|---|---|
| returns_all_context_lines_for_identical_content | src/conv/tests/diff.rs::returns_all_context_lines_for_identical_content |
| returns_added_lines_for_added_file | src/conv/tests/diff.rs::returns_added_lines_for_added_file |
| returns_removed_lines_for_deleted_file | src/conv/tests/diff.rs::returns_removed_lines_for_deleted_file |
| returns_mixed_adds_removes_and_context_for_modified_file | src/conv/tests/diff.rs::returns_mixed_adds_removes_and_context_for_modified_file |
| reads_removed_and_added_lines_that_look_like_file_headers_as_lines | src/conv/tests/diff.rs::reads_removed_and_added_lines_that_look_like_file_headers_as_lines |
| keeps_line_numbers_when_a_blank_context_line_is_an_empty_line | src/conv/tests/diff.rs::keeps_line_numbers_when_a_blank_context_line_is_an_empty_line |
| keeps_the_lines_of_a_deleted_file_that_start_with_a_dash_or_a_space | src/conv/tests/diff.rs::keeps_the_lines_of_a_deleted_file_that_start_with_a_dash_or_a_space |
| (it.each) it.each([ | src/conv/tests/diff.rs::names_the_file_without_a_prefix_for_each_way_git_writes_one |
| names_a_new_file_under_mnemonic_prefixes | src/conv/tests/diff.rs::names_a_new_file_under_mnemonic_prefixes |
| keeps_a_folder_named_like_a_prefix_when_git_writes_no_prefix | src/conv/tests/diff.rs::keeps_a_folder_named_like_a_prefix_when_git_writes_no_prefix |
| always rebuilds both sides from the lines, for random edits | src/conv/tests/diff.rs::always_rebuilds_both_sides_from_the_lines_for_random_edits |
| diffs an 8,000-line file with over a thousand changed lines quickly | src/conv/tests/diff.rs::diffs_an_8000_line_file_with_over_a_thousand_changed_lines_quickly |

## lib/__tests__/export-markdown.test.ts

| Fleet case | Rust test or owner |
|---|---|
| writes the default export: tools as one line, no thinking, short paths, times and model | src/conv/tests/export.rs::writes_the_default_export_tools_as_one_line_no_thinking_short_paths_times_and_model |
| never carries file bytes and leaves out a compaction's summary message | src/conv/tests/export.rs::never_carries_file_bytes_and_leaves_out_a_compactions_summary_message |
| leaves tool calls out, but keeps the page the agent showed | src/conv/tests/export.rs::leaves_tool_calls_out_but_keeps_the_page_the_agent_showed |
| adds each call's output, a diff for an edit and a text block for a command, with tool output on | src/conv/tests/export.rs::adds_each_calls_output_a_diff_for_an_edit_and_a_text_block_for_a_command_with_tool_output_on |
| quotes thinking when asked | src/conv/tests/export.rs::quotes_thinking_when_asked |
| keeps full paths and drops times and model when those are off | src/conv/tests/export.rs::keeps_full_paths_and_drops_times_and_model_when_those_are_off |
| counts the lines that look like keys, and rewrites nothing | src/conv/tests/export.rs::counts_the_lines_that_look_like_keys_and_rewrites_nothing |
| counts the turns that were written | src/conv/tests/export.rs::counts_the_turns_that_were_written |
| opens a fence longer than any backticks in the output | src/conv/tests/export.rs::opens_a_fence_longer_than_any_backticks_in_the_output |
| cuts a very long output and says so | src/conv/tests/export.rs::cuts_a_very_long_output_and_says_so |
| finds an edit's diff in OpenCode 2's per-file patches, or builds one from the swapped strings | src/conv/tests/export.rs::finds_an_edits_diff_in_opencode_2s_per_file_patches_or_builds_one_from_the_swapped_strings |
| writes an agent's steps, which arrive as messages of their own, as one turn | src/conv/tests/export.rs::writes_an_agents_steps_which_arrive_as_messages_of_their_own_as_one_turn |
| keeps a long path whole | src/conv/tests/export.rs::keeps_a_long_path_whole |
| shows a slash command as the command, not its expanded template | src/conv/tests/export.rs::shows_a_slash_command_as_the_command_not_its_expanded_template |
| is true only when a message has reasoning with text | src/conv/tests/export.rs::is_true_only_when_a_message_has_reasoning_with_text |
| recognises common key shapes and ignores plain text | src/conv/tests/export.rs::recognises_common_key_shapes_and_ignores_plain_text |
| names the file after the title | src/conv/tests/export.rs::names_the_file_after_the_title |

## lib/__tests__/late-tool-update.test.ts

| Fleet case | Rust test or owner |
|---|---|
| updates the existing part of the older message, not appending | src/fleet/tests/work.rs::updates_the_existing_part_of_the_older_message_not_appending |
| same call id arriving under a different part id does not duplicate | src/fleet/tests/work.rs::same_call_id_arriving_under_a_different_part_id_does_not_duplicate |

## lib/__tests__/reopened-session.test.ts

| Fleet case | Rust test or owner |
|---|---|
| links the sub-agent card to its delegation | src/fleet/tests/delegations.rs::links_the_sub_agent_card_to_its_delegation |
| keeps a generic tool card and the canvas card | src/conv/tests/sweep.rs::reopened_session::keeps_a_generic_tool_card_and_the_canvas_card |
| updates a tool still running when reopened instead of adding a second card | src/fleet/tests/delegations.rs::updates_a_tool_still_running_when_reopened_instead_of_adding_a_second_card |

## lib/__tests__/retry-status.test.ts

| Fleet case | Rust test or owner |
|---|---|
| says the attempt out of how many, when and why | src/conv/tests/sweep.rs::retry_status::says_the_attempt_out_of_how_many_when_and_why |
| leaves out what the harness didn't say (OpenCode 2 gives no maximum) | src/conv/tests/working.rs::retry_status::leaves_out_what_the_harness_didnt_say |
| counts down in seconds, then minutes, then says now | src/conv/tests/sweep.rs::retry_status::counts_down_in_seconds_then_minutes_then_says_now |
| is short on a session row | belongs to component 6: session list (`retryShortLabel`, the row's Retry 3/10) |

## lib/__tests__/running-work.test.ts

| Fleet case | Rust test or owner |
|---|---|
| reads the server's item, filling in what it leaves out | src/fleet/tests/work.rs::reads_the_servers_item_filling_in_what_it_leaves_out |
| drops what isn't an item and reads an unknown kind as a task | src/fleet/tests/work.rs::drops_what_isnt_an_item_and_reads_an_unknown_kind_as_a_task |
| replaces an item by id, keeps ended work ended, and returns the same list when nothing changed | src/fleet/tests/work.rs::replaces_an_item_by_id_keeps_ended_work_ended_and_returns_the_same_list_when_nothing_changed |
| keeps items oldest first | src/fleet/tests/work.rs::keeps_items_oldest_first |
| says how it ended in a few words | src/fleet/tests/work.rs::says_how_long_and_how_it_ended_in_a_few_words |
| starts from the snapshot's runningWork | src/fleet/tests/work.rs::starts_from_the_snapshots_runningwork |
| starts with no work when the snapshot has none | src/fleet/tests/work.rs::starts_with_no_work_when_the_snapshot_has_none |
| adds work on work.started, changes it on work.updated and ends it on work.ended | src/fleet/tests/work.rs::adds_work_on_work_started_changes_it_on_work_updated_and_ends_it_on_work_ended |
| doesn't count background work as the session's own | src/fleet/tests/work.rs::doesnt_count_background_work_as_the_sessions_own |
| keeps the same state for an event it already has | src/fleet/tests/work.rs::keeps_the_same_state_for_an_event_it_already_has |
| is the prompt that began the turn; a prompt sent mid-turn doesn't restart the clock | src/fleet/tests/work.rs::is_the_prompt_that_began_the_turn_a_prompt_sent_mid_turn_doesnt_restart_the_clock |

## lib/__tests__/session-interruption.test.ts

| Fleet case | Rust test or owner |
|---|---|
| reads a full interruption | src/conv/tests/sweep.rs::session_interruption::to_session_interruption_reads_a_full_interruption |
| fills in what is missing | src/conv/tests/sweep.rs::session_interruption::to_session_interruption_fills_in_what_is_missing |
| is null without a usable time or an object | src/conv/tests/sweep.rs::session_interruption::to_session_interruption_is_null_without_a_usable_time_or_an_object |
| names the row by what was lost | belongs to component 6: session list (`interruptionRowLabel`, "Cut off by a restart") |
| counts files | src/conv/tests/sweep.rs::session_interruption::wording_counts_files + src/conv/tests/working.rs::interrupted_card::says_what_was_working_the_last_call_and_the_changed_files |


## lib/__tests__/session-messages.test.ts

| Fleet case | Rust test or owner |
|---|---|
| returns the sender and the text of a message another session sent | src/conv/tests/peer.rs::parse_peer_message_cases::returns_the_sender_and_the_text_of_a_message_another_session_sent |
| keeps the text's own lines and decodes the title Fleet encoded | src/conv/tests/peer.rs::parse_peer_message_cases::keeps_the_texts_own_lines_and_decodes_the_title_fleet_encoded |
| names the sender's machine when it's on another one | src/conv/tests/peer.rs::parse_peer_message_cases::names_the_senders_machine_when_its_on_another_one |
| ignores a prompt that only mentions the tag | src/conv/tests/peer.rs::parse_peer_message_cases::ignores_a_prompt_that_only_mentions_the_tag |
| returns the session, how its turn ended and its reply | src/conv/tests/peer.rs::parse_peer_update_cases::returns_the_session_how_its_turn_ended_and_its_reply |
| names the machine of a session on another one | src/conv/tests/peer.rs::parse_peer_update_cases::names_the_machine_of_a_session_on_another_one |
| reads a failure and decodes the title | src/conv/tests/peer.rs::parse_peer_update_cases::reads_a_failure_and_decodes_the_title |
| is not a message, and a message is not an update | src/conv/tests/peer.rs::parse_peer_update_cases::is_not_a_message_and_a_message_is_not_an_update |
| drops them from the sender's title and the text | src/conv/tests/peer.rs::terminal_codes_in_a_message_from_another_session::drops_them_from_the_senders_title_and_the_text |
| drops them from an update's title and text | src/conv/tests/peer.rs::terminal_codes_in_a_message_from_another_session::drops_them_from_an_updates_title_and_text |

## lib/__tests__/shell-commands.test.ts

| Fleet case | Rust test or owner |
|---|---|
| is shell mode only when the draft starts with ! | src/composer/tests.rs::is_shell_mode_only_when_the_draft_starts_with_bang |
| runs what follows the ! | src/composer/tests.rs::runs_what_follows_the_bang |
| keeps the user's and the shell's roles and puts everything else on the agent's side | src/conv/tests/shell.rs::keeps_the_users_and_the_shells_roles_and_puts_everything_else_on_the_agents_side |
| shows a command still running | src/conv/tests/shell.rs::shows_a_command_still_running |
| reads how a command ended from its metadata | src/conv/tests/shell.rs::reads_how_a_command_ended_from_its_metadata |
| calls a command done when the harness reports no exit code | src/conv/tests/shell.rs::calls_a_command_done_when_the_harness_reports_no_exit_code |
| waits for the command's part | src/conv/tests/shell.rs::waits_for_the_commands_part |
| becomes the shell's message when its part came before the message | src/conv/tests/sweep.rs::a_shell_command_arriving_live::becomes_the_shells_message_when_its_part_came_before_the_message |
| starts as the shell's message when the message comes first | src/conv/tests/sweep.rs::a_shell_command_arriving_live::starts_as_the_shells_message_when_the_message_comes_first |

## lib/__tests__/snapshot-tool-output-hydration.test.ts

| Fleet case | Rust test or owner |
|---|---|
| hydrates completed tool state with output from snapshot payload | src/conv/tests/sweep.rs::snapshot_tool_output_hydration::hydrates_completed_tool_state_with_output_from_snapshot_payload |
| handles tool state with nested JSON output | src/conv/tests/sweep.rs::snapshot_tool_output_hydration::handles_tool_state_with_nested_json_output |
| handles tool state with error output | src/conv/tests/sweep.rs::snapshot_tool_output_hydration::handles_tool_state_with_error_output |
| shows no output for pending tool state | src/conv/tests/sweep.rs::snapshot_tool_output_hydration::shows_no_output_for_pending_tool_state |

## lib/__tests__/tool-classification.test.ts

| Fleet case | Rust test or owner |
|---|---|
| %j | src/conv/tests/classification.rs::every_name_in_the_table_is_classified_as_the_client_does_today (label, icon, heading, subagent, pattern, titled-from-answer columns; turns, fold-steps, pr-utils, question and lineage columns belong to sites this app does not have) |
| todowrite has the ListTodo icon, the heading 'Update todos' and 'N todos' as its label (the server sends Claude Code's TodoWrite lowercased) | src/conv/tests/registry.rs::has_a_label_a_header_and_a_todo_list_icon |
| `task` has no label builder (its card says 'task'), though its icon and heading exist; `subagent` has all three | src/conv/tests/registry.rs::task_has_no_label_builder_though_its_icon_and_heading_exist_subagent_has_all_three |
| edit-like tools share the Pencil; none is assumed to create a file (only the session diff says a file is new) | src/conv/tests/registry.rs::edit_like_tools_share_the_pencil_and_label_by_their_file |
| shell-like tools: `terminal` is a shell everywhere; `execute` keeps the Code icon and is not a command in turns, fold-steps or pr-utils | src/conv/tests/registry.rs::shell_like_tools_terminal_is_a_shell_and_execute_keeps_the_code_icon |
| read, search and web tools: list and LS get a Folder, codesearch a Search; fold-steps files list, webfetch under read, websearch and codesearch under search | src/conv/tests/registry.rs::read_search_and_web_tools_list_and_ls_get_a_folder_codesearch_a_search |
| case does not matter anywhere: Claude Code's own casing gets the same label, icon and heading as the lowercase name | src/conv/tests/registry.rs::case_does_not_matter_anywhere |
| `Agent` is a subagent on the desktop card too, drawn as a Task | src/conv/tests/registry.rs::agent_is_a_subagent_drawn_as_a_task |
| the subagent set: task, agent and subagent, either case | src/conv/tests/registry.rs::the_subagent_set_is_task_agent_and_subagent_either_case |
| the pattern pill is glob and grep, in any case | src/conv/tests/registry.rs::the_pattern_pill_is_glob_and_grep_in_any_case |
| Fleet's titled tools take their title from Fleet's answer; the memory and browser read/act tools do not | src/conv/tests/registry.rs::fleets_titled_tools_take_their_title_from_fleets_answer_the_memory_and_browser_read_act_tools_do_not |
| `mcp__fleet__x` is Fleet's tool x everywhere; another server's tool is unknown and shown as its own name | src/conv/tests/registry.rs::mcp_fleet_x_is_fleets_tool_x_everywhere_another_servers_tool_is_unknown_and_shown_as_its_own_name |
| the question checks match `question` exactly | src/conv/tests/registry.rs::the_question_checks_match_question_exactly |

## lib/__tests__/tool-labels.test.ts

| Fleet case | Rust test or owner |
|---|---|
| labels OpenCode's tools from their input | src/conv/tests/registry.rs::labels_opencodes_tools_from_their_input |
| labels OpenCode 2's shell like OpenCode's bash | src/conv/tests/registry.rs::labels_opencode_2s_shell_like_opencodes_bash |
| reads the file OpenCode 2 names `path` | src/conv/tests/registry.rs::reads_the_file_opencode_2_names_path |
| labels OpenCode 2's other tools | src/conv/tests/registry.rs::labels_opencode_2s_other_tools |
| labels a shown page by its tab and file | src/conv/tests/registry.rs::labels_a_shown_page_by_its_tab_and_file |
| labels a walkthrough by its title | src/conv/tests/registry.rs::labels_a_walkthrough_by_its_title |
| names reading an @-referenced session | src/conv/tests/registry.rs::names_reading_an_at_referenced_session |
| falls back to the tool's name when the input says nothing | src/conv/tests/registry.rs::falls_back_to_the_tools_name_when_the_input_says_nothing |
| names OpenCode 2's tools and gives them the icon of the OpenCode tool they replace | src/conv/tests/registry.rs::names_opencode_2s_tools_and_gives_them_the_icon_of_the_opencode_tool_they_replace |
| shows OpenCode 2's command and output | src/conv/tests/sweep.rs::a_shell_tool_card::shows_opencode_2s_command_and_output |
| is the shell tool in both OpenCodes | src/conv/tests/sweep.rs::is_bash_tool::is_the_shell_tool_in_both_opencodes |

## lib/__tests__/tool-output.test.ts

| Fleet case | Rust test or owner |
|---|---|
| keeps plain text as it is, one line per line | src/conv/tests/output.rs::keeps_plain_text_as_it_is_one_line_per_line |
| has no lines for empty or blank output | src/conv/tests/output.rs::has_no_lines_for_empty_or_blank_output |
| turns colour codes into classes in Fleet's palette, and resets them | src/conv/tests/output.rs::turns_colour_codes_into_classes_in_fleets_palette_and_resets_them |
| carries a colour across lines until it's reset | src/conv/tests/output.rs::carries_a_colour_across_lines_until_its_reset |
| reads bright, background, dim, italic and underline codes | src/conv/tests/output.rs::reads_bright_background_dim_italic_and_underline_codes |
| draws 256-colour and true-colour codes, palette indexes as classes and the rest as a colour | src/conv/tests/output.rs::draws_256_colour_and_true_colour_codes_palette_indexes_as_classes_and_the_rest_as_a_colour |
| drops cursor moves, erases, private modes, titles and charset picks | src/conv/tests/output.rs::drops_cursor_moves_erases_private_modes_titles_and_charset_picks |
| keeps only the last frame of a line rewritten with \\r | src/conv/tests/output.rs::keeps_only_the_last_frame_of_a_line_rewritten_with_cr |
| keeps a frame a trailing \\r would otherwise wipe, and the colour set in a dropped frame | src/conv/tests/output.rs::keeps_a_frame_a_trailing_cr_would_otherwise_wipe_and_the_colour_set_in_a_dropped_frame |
| treats \\r\\n as a plain line end | src/conv/tests/output.rs::treats_crlf_as_a_plain_line_end |
| drops a broken escape instead of showing it, and keeps the text around it | src/conv/tests/output.rs::drops_a_broken_escape_instead_of_showing_it_and_keeps_the_text_around_it |
| drops bells, backspaces and the other control characters, but keeps tabs | src/conv/tests/output.rs::drops_bells_backspaces_and_the_other_control_characters_but_keeps_tabs |
| never turns the output into markup | src/conv/tests/output.rs::never_turns_the_output_into_markup |
| parses 5,000 coloured lines quickly | src/conv/tests/output.rs::parses_5000_coloured_lines_quickly |
| escapes every bit of text, leaves uncoloured text bare and wraps coloured text in Fleet's span | DOM only: builds an HTML string with spans; Rune draws spans directly (src/conv/tests/output.rs::never_turns_the_output_into_markup) |
| parses the same output once | src/conv/tests/output.rs::parses_the_same_output_once |

## lib/__tests__/tool-row.test.ts

| Fleet case | Rust test or owner |
|---|---|
| says tenths under a second, then seconds, then minutes | src/conv/tests/tool_row.rs::says_tenths_under_a_second_then_seconds_then_minutes |
| is empty when nothing was timed | src/conv/tests/tool_row.rs::is_empty_when_nothing_was_timed |
| joins the command, exit code and time | src/conv/tests/tool_row.rs::joins_the_command_exit_code_and_time |
| leaves out what the harness did not report | src/conv/tests/tool_row.rs::leaves_out_what_the_harness_did_not_report |
| collapses two or more completed reads in a row | src/conv/tests/tool_row.rs::collapses_two_or_more_completed_reads_in_a_row |
| takes reads still streaming into the row, whether or not their path has arrived | src/conv/tests/tool_row.rs::takes_reads_still_streaming_into_the_row_whether_or_not_their_path_has_arrived |
| leaves a single read and a failed read as their own rows | src/conv/tests/tool_row.rs::leaves_a_single_read_and_a_failed_read_as_their_own_rows |
| works for any harness's read tool name | src/conv/tests/tool_row.rs::works_for_any_harnesss_read_tool_name |

## lib/__tests__/turn-fold.test.ts

| Fleet case | Rust test or owner |
|---|---|
| keeps the latest turn open after it ends when you watched it (keepLast), and folds it once a newer turn exists | src/conv/tests/fold.rs::keeps_the_latest_turn_open_after_it_ends_when_you_watched_it_keep_last_and_folds_it_once_a_newer_turn_exists |
| folds a finished turn: tool rows and thinking go, the answer stays, the fold row appears | src/conv/tests/fold.rs::folds_a_finished_turn_tool_rows_and_thinking_go_the_answer_stays_the_fold_row_appears |
| never hides the agent's text: narration between calls stays, in order, and only tool rows fold | src/conv/tests/fold.rs::never_hides_the_agents_text_narration_between_calls_stays_in_order_and_only_tool_rows_fold |
| keeps the reply to a steered message visible in its own turn | src/conv/tests/fold.rs::keeps_the_reply_to_a_steered_message_visible_in_its_own_turn |
| keeps the last text as the answer when all of it came before the last call | src/conv/tests/fold.rs::keeps_the_last_text_as_the_answer_when_all_of_it_came_before_the_last_call |
| leaves the running turn open | src/conv/tests/fold.rs::leaves_the_running_turn_open |
| folds every finished turn, the latest one included, but only the running one stays open | src/conv/tests/fold.rs::folds_every_finished_turn_the_latest_one_included_but_only_the_running_one_stays_open |
| keeps failed rows visible and counts them | src/conv/tests/fold.rs::keeps_failed_rows_visible_and_counts_them |
| keeps sub-agent rows, pages and screenshots visible | src/conv/tests/fold.rs::keeps_sub_agent_rows_pages_and_screenshots_visible |
| shows everything again when the turn is opened, with the fold row kept | src/conv/tests/fold.rs::shows_everything_again_when_the_turn_is_opened_with_the_fold_row_kept |
| leaves a turn with nothing to fold as it is | src/conv/tests/fold.rs::leaves_a_turn_with_nothing_to_fold_as_it_is |
| counts several edits of one file as one file edited | src/conv/tests/fold.rs::counts_several_edits_of_one_file_as_one_file_edited |
| puts nothing after the answer of a turn that changed files: no changed-files card | src/conv/tests/fold.rs::puts_nothing_after_the_answer_of_a_turn_that_changed_files_no_changed_files_card |
| counts the file's real change from the session diff, not every line written | src/conv/tests/fold.rs::counts_the_files_real_change_from_the_session_diff_not_every_line_written |
| draws no counts while the session diff isn't loaded, rather than a wrong +28 | src/conv/tests/fold.rs::draws_no_counts_while_the_session_diff_isnt_loaded_rather_than_a_wrong_plus_28 |
| counts a file the session diff doesn't list as unchanged | src/conv/tests/fold.rs::counts_a_file_the_session_diff_doesnt_list_as_unchanged |
| counts a file written twice as one file edited | src/conv/tests/fold.rs::counts_a_file_written_twice_as_one_file_edited |
| keeps a Show page row out of the fold even when the call carries no page, and doesn't count it | src/conv/tests/fold.rs::keeps_a_show_page_row_out_of_the_fold_even_when_the_call_carries_no_page_and_doesnt_count_it |
| keeps every row that opens something (app run, browser, walkthrough, canvas) out of the fold, and doesn't count it | src/conv/tests/fold.rs::keeps_every_row_that_opens_something_app_run_browser_walkthrough_canvas_out_of_the_fold_and_doesnt_count_it |
| doesn't count ExitPlanMode as a read | src/conv/tests/fold.rs::doesnt_count_exit_plan_mode_as_a_read |
| marks a turn that failed or was stopped, so no ✓ is drawn for it | src/conv/tests/fold.rs::marks_a_turn_that_failed_or_was_stopped_so_no_tick_is_drawn_for_it |
| ends 'Worked for' with the turn's own work, not a later background notice or retry | src/conv/tests/fold.rs::ends_worked_for_with_the_turns_own_work_not_a_later_background_notice_or_retry |
| relativises Windows paths (either slash, any drive case) so a write row finds its stats | src/conv/tests/fold.rs::relativises_windows_paths_either_slash_any_drive_case_so_a_write_row_finds_its_stats |
| doesn't recount an earlier turn from today's diff: unknown draws no counts | src/conv/tests/fold.rs::doesnt_recount_an_earlier_turn_from_todays_diff_unknown_draws_no_counts |
| uses the snapshot the server took as the turn ended, whatever today's diff says | src/conv/tests/fold.rs::uses_the_snapshot_the_server_took_as_the_turn_ended_whatever_todays_diff_says |
| never takes a later turn's snapshot for an earlier turn | src/conv/tests/fold.rs::never_takes_a_later_turns_snapshot_for_an_earlier_turn |
| a snapshot that lists no such file means the turn left it unchanged | src/conv/tests/fold.rs::a_snapshot_that_lists_no_such_file_means_the_turn_left_it_unchanged |
| a file outside the session's folder is unknown, not +0 −0 | src/conv/tests/fold.rs::a_file_outside_the_sessions_folder_is_unknown_not_plus_0_minus_0 |

## lib/__tests__/turn-retry.test.ts

| Fleet case | Rust test or owner |
|---|---|
| names each limit, and nothing else | src/conv/tests/retry.rs::limit_title_names_each_limit_and_nothing_else |
| reads the server's shape and refuses anything else | src/conv/tests/retry.rs::to_scheduled_retry_reads_the_servers_shape_and_refuses_anything_else |

## lib/__tests__/working-on.test.ts

| Fleet case | Rust test or owner |
|---|---|
| is null with nothing running | src/conv/tests/working.rs::working_on::is_none_with_nothing_running |
| names the one running tool and how long it has run | src/conv/tests/working.rs::names_the_one_running_tool_and_how_long_it_has_run |
| names the longest-running one and counts the rest | src/conv/tests/working.rs::names_the_longest_running_one_and_counts_the_rest |
| says the steered message waits when it sits below the running call on a next-step harness | src/conv/tests/working.rs::says_the_steered_message_waits_when_it_sits_below_the_running_call_on_a_next_step_harness |
| says nothing extra on a harness that reads a steer at once, or one that doesn't say | src/conv/tests/working.rs::says_nothing_extra_on_a_harness_that_reads_a_steer_at_once_or_one_that_doesnt_say |
| doesn't hold a steered message that was sent before the call started | src/conv/tests/working.rs::doesnt_hold_a_steered_message_that_was_sent_before_the_call_started |
| doesn't hold a message that isn't steered | src/conv/tests/working.rs::doesnt_hold_a_message_that_isnt_steered |
| falls back to the message's start when the call has none | src/conv/tests/working.rs::falls_back_to_the_messages_start_when_the_call_has_none |

## lib/tools/__tests__/registry.test.ts

| Fleet case | Rust test or owner |
|---|---|
| finds a tool by its name, an alias, and in any case | src/conv/tests/registry.rs::finds_a_tool_by_its_name_an_alias_and_in_any_case |
| keeps the name as it arrived | src/conv/tests/registry.rs::keeps_the_name_as_it_arrived |
| claims every name once | src/conv/tests/registry.rs::claims_every_name_once |
| resolves Fleet's own MCP server to Fleet's tools | src/conv/tests/registry.rs::resolves_fleets_own_mcp_server_to_fleets_tools |
| knows nothing of another MCP server's tools, but reads their names | src/conv/tests/registry.rs::knows_nothing_of_another_mcp_servers_tools_but_reads_their_names |
| an MCP server cannot pose as a built-in tool | src/conv/tests/registry.rs::an_mcp_server_cannot_pose_as_a_built_in_tool |
| parses MCP names | src/conv/tests/registry.rs::parses_mcp_names |
| falls back for a tool it does not know | src/conv/tests/registry.rs::falls_back_for_a_tool_it_does_not_know |
| copes with no name at all | src/conv/tests/registry.rs::copes_with_no_name_at_all |
| treats any other fleet_ tool as Fleet's | src/conv/tests/registry.rs::treats_any_other_fleet_tool_as_fleets |
| ${category}: ${names.join(", ")} | src/conv/tests/registry.rs::each_name_is_in_its_category |
| file writes: write and notebooks create, the other edits modify | src/conv/tests/registry.rs::file_writes_write_and_notebooks_create_the_other_edits_modify |
| flags the tools whose card title is Fleet's answer, and those that drive the browser | src/conv/tests/registry.rs::flags_the_tools_whose_card_title_is_fleets_answer_and_those_that_drive_the_browser |
| ${kind}: ${names.join(", ")} | src/conv/tests/registry.rs::each_name_is_classified_as_permissions_classify_does |
| decides the two names the client knows and the server does not | src/conv/tests/registry.rs::decides_the_two_names_the_client_knows_and_the_server_does_not |
| labels every known tool as tool-labels does | src/conv/tests/registry.rs::labels_every_known_tool_from_the_same_inputs_without_panicking |
| falls back to the tool name where tool-labels has no case | src/conv/tests/registry.rs::falls_back_to_the_tool_name_where_tool_labels_has_no_case |
| labels the tools that take a file or folder by it, whichever field the harness used | src/conv/tests/registry.rs::labels_the_tools_that_take_a_file_or_folder_by_it_whichever_field_the_harness_used |
| draws every tool the icon map knows with the same icon and header | src/conv/tests/classification.rs::every_name_in_the_table_is_classified_as_the_client_does_today (Rust has no second icon map to compare with; the table pins each name's icon and heading) |
| gives the headers a tool without an entry gets today | src/conv/tests/classification.rs::every_name_in_the_table_is_classified_as_the_client_does_today (headings of the unknown names) |
| words permission asks as the phone does | belongs to component 4: questions and permissions (`permissionHeading`, `dontAskAgainWording`) |
| has a label, a header and a todo-list icon | src/conv/tests/registry.rs::has_a_label_a_header_and_a_todo_list_icon |
| reads the file under any harness's name | src/conv/tests/registry.rs::reads_the_file_under_any_harnesss_name |
| reads the common fields and ignores blanks and wrong types | src/conv/tests/registry.rs::reads_the_common_fields_and_ignores_blanks_and_wrong_types |
| takes a skill's id when it has no name | src/conv/tests/registry.rs::takes_a_skills_id_when_it_has_no_name |
| reads questions | src/conv/tests/registry.rs::reads_questions |
| copes with anything that is not a record | src/conv/tests/registry.rs::copes_with_anything_that_is_not_a_record |
| asRecord accepts only plain objects | src/conv/tests/registry.rs::as_record_accepts_only_plain_objects |
| reads as role, model and time once the pass has finished | src/conv/tests/registry.rs::reads_as_role_model_and_time_once_the_pass_has_finished |
| adds a judge's or critic's verdict, the credits it cost and why a failed pass failed | src/conv/tests/registry.rs::adds_a_judges_or_critics_verdict_the_credits_it_cost_and_why_a_failed_pass_failed |
| heads a turn with its workflow, credits and requests | src/conv/tests/registry.rs::heads_a_turn_with_its_workflow_credits_and_requests |

## stores/__tests__/agent-browser.test.ts

| Fleet case | Rust test or owner |
|---|---|
| lists a call's steps in order, once each | src/conv/tests/browser.rs::lists_a_calls_steps_in_order_once_each |
| keeps the agent's tabs from its steps: opened, moved and closed | belongs to component 10: content panel (the agent's tabs: opened, moved, closed) | **Closed by component 10 (18 November 2026):** ported (10 pieces C and M).
| a failed step doesn't move the tab | belongs to component 10: content panel (the agent's tabs) |
| finds the agent's tab for the page a canvas shows, whatever loopback name either uses | belongs to component 10: content panel (a canvas's tab, loopback names) |
| keeps steps that arrived while the session was loading | src/conv/tests/browser.rs::keeps_steps_that_arrived_while_the_session_was_loading |
| same app means same scheme, port and machine | belongs to component 10: content panel (`sameApp` for the browser canvas) |

## Counts

{'ported': 400, 'belongs': 54, 'dom': 23, 'na': 2, 'open': 8}
