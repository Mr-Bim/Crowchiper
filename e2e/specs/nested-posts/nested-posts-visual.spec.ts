import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../../utils/fixtures.ts";
import { getServer, Server } from "../../utils/server.ts";
import { BrowserContext } from "@playwright/test";
import {
  createPostWithTitle,
  getPostByTitle,
  dragPostToPost,
} from "./nested-posts-helpers.ts";

test.describe("Nested posts - Visual hierarchy", () => {
  test.describe.configure({ mode: "serial" });

  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `nested_visual_${uniqueTestId()}`;
    userResult = await createUser({
      context,
      baseUrl,
      username,
      enableEncryption: true,
    });
  });

  test.afterAll(async () => {
    await context?.close();
  });

  test("nested posts have correct indentation", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Wait for initial post
    await expect(postList.locator(".post-wrapper")).toHaveCount(1, {
      timeout: 10000,
    });

    // Create hierarchy: Root -> Level 1 -> Level 2
    await createPostWithTitle(page, "Indent Root");
    await createPostWithTitle(page, "Indent Level 1");
    await createPostWithTitle(page, "Indent Level 2");

    // Drag Level 1 under Root
    await dragPostToPost(page, "Indent Level 1", "Indent Root", {
      centerTarget: true,
    });

    // Drag Level 2 under Level 1
    await dragPostToPost(page, "Indent Level 2", "Indent Level 1", {
      centerTarget: true,
    });

    // Verify indentation via padding-left style
    const root = getPostByTitle(page, "Indent Root");
    const rootContainer = root.locator(".post-item-container");
    const level1Container = getPostByTitle(page, "Indent Level 1").locator(
      ".post-item-container",
    );
    const level2Container = getPostByTitle(page, "Indent Level 2").locator(
      ".post-item-container",
    );

    // Root (depth 0) should have 0px padding
    const rootPadding = await rootContainer.evaluate(
      (el) => getComputedStyle(el).paddingLeft,
    );
    expect(rootPadding).toBe("0px");

    // Level 1 (depth 1) should have 16px padding
    const level1Padding = await level1Container.evaluate(
      (el) => getComputedStyle(el).paddingLeft,
    );
    expect(level1Padding).toBe("16px");

    // Level 2 (depth 2) should have 32px padding
    const level2Padding = await level2Container.evaluate(
      (el) => getComputedStyle(el).paddingLeft,
    );
    expect(level2Padding).toBe("32px");
  });

  test("chevron rotates when expanded/collapsed", async () => {
    const { page } = userResult;

    // Use the hierarchy we just created
    const root = getPostByTitle(page, "Indent Root");
    const expandBtn = root.locator(".post-expand-btn");
    const chevron = expandBtn.locator(".chevron");

    // Ensure expanded
    await expect(expandBtn).toHaveAttribute("data-expanded", "true");

    // Get initial transform (should be rotated)
    const expandedTransform = await chevron.evaluate(
      (el) => getComputedStyle(el).transform,
    );

    // Collapse
    await expandBtn.click();
    await page.waitForTimeout(300);

    const collapsedTransform = await chevron.evaluate(
      (el) => getComputedStyle(el).transform,
    );

    // Transforms should be different (one rotated, one not)
    expect(expandedTransform).not.toBe(collapsedTransform);

    // Expand again and verify transform returns
    await expandBtn.click();
    await page.waitForTimeout(300);

    const expandedAgainTransform = await chevron.evaluate(
      (el) => getComputedStyle(el).transform,
    );
    expect(expandedAgainTransform).toBe(expandedTransform);
  });
});
