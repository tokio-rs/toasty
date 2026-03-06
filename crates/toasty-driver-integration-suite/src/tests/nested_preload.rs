use crate::prelude::*;

// ===== HasMany -> HasOne<Option<T>> =====
// User has_many Posts, each Post has_one optional Detail
#[ignore] // TODO: nested preload panics with type mismatch for HasMany -> HasOne<Option<T>>
#[driver_test(id(ID))]
pub async fn nested_has_many_then_has_one_optional(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        posts: toasty::HasMany<Post>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_one]
        detail: toasty::HasOne<Option<Detail>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Detail {
        #[key]
        #[auto]
        id: ID,

        body: String,

        #[unique]
        post_id: Option<ID>,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::BelongsTo<Option<Post>>,
    }

    let mut db = test.setup_db(models!(User, Post, Detail)).await;

    let user = User::create()
        .name("Alice")
        .post(
            Post::create()
                .title("P1")
                .detail(Detail::create().body("D1")),
        )
        .post(Post::create().title("P2")) // no detail
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().posts().detail())
        .get(&mut db)
        .await?;

    let posts = user.posts.get();
    assert_eq!(2, posts.len());

    let mut with_detail = 0;
    let mut without_detail = 0;
    for post in posts {
        match post.detail.get() {
            Some(d) => {
                assert_eq!("D1", d.body);
                with_detail += 1;
            }
            None => without_detail += 1,
        }
    }
    assert_eq!(1, with_detail);
    assert_eq!(1, without_detail);

    Ok(())
}

// ===== HasMany -> HasOne<T> (required) =====
// User has_many Accounts, each Account has_one required Settings
#[ignore] // TODO: nested preload panics with type mismatch for HasMany -> HasOne<T>
#[driver_test(id(ID))]
pub async fn nested_has_many_then_has_one_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        accounts: toasty::HasMany<Account>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,

        label: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_one]
        settings: toasty::HasOne<Settings>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Settings {
        #[key]
        #[auto]
        id: ID,

        theme: String,

        #[unique]
        account_id: Option<ID>,

        #[belongs_to(key = account_id, references = id)]
        account: toasty::BelongsTo<Option<Account>>,
    }

    let mut db = test.setup_db(models!(User, Account, Settings)).await;

    let user = User::create()
        .name("Bob")
        .account(
            Account::create()
                .label("A1")
                .settings(Settings::create().theme("dark")),
        )
        .account(
            Account::create()
                .label("A2")
                .settings(Settings::create().theme("light")),
        )
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().accounts().settings())
        .get(&mut db)
        .await?;

    let accounts = user.accounts.get();
    assert_eq!(2, accounts.len());

    let mut themes: Vec<&str> = accounts
        .iter()
        .map(|a| a.settings.get().theme.as_str())
        .collect();
    themes.sort();
    assert_eq!(themes, vec!["dark", "light"]);

    Ok(())
}

// ===== HasMany -> BelongsTo<T> (required) =====
// Category has_many Items, each Item belongs_to a Brand
#[driver_test(id(ID))]
pub async fn nested_has_many_then_belongs_to_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Category {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        items: toasty::HasMany<Item>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Brand {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        category_id: ID,

        #[belongs_to(key = category_id, references = id)]
        category: toasty::BelongsTo<Category>,

        #[index]
        brand_id: ID,

        #[belongs_to(key = brand_id, references = id)]
        brand: toasty::BelongsTo<Brand>,
    }

    let mut db = test.setup_db(models!(Category, Brand, Item)).await;

    let brand_a = Brand::create().name("BrandA").exec(&mut db).await?;
    let brand_b = Brand::create().name("BrandB").exec(&mut db).await?;

    let cat = Category::create()
        .name("Electronics")
        .item(Item::create().title("Phone").brand(&brand_a))
        .item(Item::create().title("Laptop").brand(&brand_b))
        .exec(&mut db)
        .await?;

    let cat = Category::filter_by_id(cat.id)
        .include(Category::fields().items().brand())
        .get(&mut db)
        .await?;

    let items = cat.items.get();
    assert_eq!(2, items.len());

    let mut brand_names: Vec<&str> = items.iter().map(|i| i.brand.get().name.as_str()).collect();
    brand_names.sort();
    assert_eq!(brand_names, vec!["BrandA", "BrandB"]);

    Ok(())
}

// ===== HasMany -> BelongsTo<Option<T>> =====
// Team has_many Tasks, each Task optionally belongs_to an Assignee
#[ignore] // TODO: nested preload panics with type mismatch for HasMany -> BelongsTo<Option<T>>
#[driver_test(id(ID))]
pub async fn nested_has_many_then_belongs_to_optional(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Team {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        tasks: toasty::HasMany<Task>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Assignee {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Task {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        team_id: ID,

        #[belongs_to(key = team_id, references = id)]
        team: toasty::BelongsTo<Team>,

        #[index]
        assignee_id: Option<ID>,

        #[belongs_to(key = assignee_id, references = id)]
        assignee: toasty::BelongsTo<Option<Assignee>>,
    }

    let mut db = test.setup_db(models!(Team, Assignee, Task)).await;

    let person = Assignee::create().name("Alice").exec(&mut db).await?;

    let team = Team::create()
        .name("Engineering")
        .task(Task::create().title("Assigned").assignee(&person))
        .task(Task::create().title("Unassigned"))
        .exec(&mut db)
        .await?;

    let team = Team::filter_by_id(team.id)
        .include(Team::fields().tasks().assignee())
        .get(&mut db)
        .await?;

    let tasks = team.tasks.get();
    assert_eq!(2, tasks.len());

    let mut assigned = 0;
    let mut unassigned = 0;
    for task in tasks {
        match task.assignee.get() {
            Some(a) => {
                assert_eq!("Alice", a.name);
                assigned += 1;
            }
            None => unassigned += 1,
        }
    }
    assert_eq!(1, assigned);
    assert_eq!(1, unassigned);

    Ok(())
}

// ===== HasOne<Option<T>> -> HasMany =====
// User has_one optional Profile, Profile has_many Badges
#[ignore] // TODO: nested preload panics with type mismatch for HasOne<Option<T>> -> HasMany
#[driver_test(id(ID))]
pub async fn nested_has_one_optional_then_has_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        bio: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        #[has_many]
        badges: toasty::HasMany<Badge>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Badge {
        #[key]
        #[auto]
        id: ID,

        label: String,

        #[index]
        profile_id: ID,

        #[belongs_to(key = profile_id, references = id)]
        profile: toasty::BelongsTo<Profile>,
    }

    let mut db = test.setup_db(models!(User, Profile, Badge)).await;

    // User with profile and badges
    let user = User::create()
        .name("Alice")
        .profile(
            Profile::create()
                .bio("hi")
                .badge(Badge::create().label("Gold"))
                .badge(Badge::create().label("Silver")),
        )
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().profile().badges())
        .get(&mut db)
        .await?;

    let profile = user.profile.get().as_ref().unwrap();
    assert_eq!("hi", profile.bio);
    let mut labels: Vec<&str> = profile
        .badges
        .get()
        .iter()
        .map(|b| b.label.as_str())
        .collect();
    labels.sort();
    assert_eq!(labels, vec!["Gold", "Silver"]);

    // User without profile - nested preload should handle gracefully
    let user2 = User::create().name("Bob").exec(&mut db).await?;

    let user2 = User::filter_by_id(user2.id)
        .include(User::fields().profile().badges())
        .get(&mut db)
        .await?;

    assert!(user2.profile.get().is_none());

    Ok(())
}

// ===== HasOne<T> (required) -> HasMany =====
// Order has_one required Invoice, Invoice has_many LineItems
#[ignore] // TODO: nested preload panics with type mismatch for HasOne<T> -> HasMany
#[driver_test(id(ID))]
pub async fn nested_has_one_required_then_has_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Order {
        #[key]
        #[auto]
        id: ID,

        label: String,

        #[has_one]
        invoice: toasty::HasOne<Invoice>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Invoice {
        #[key]
        #[auto]
        id: ID,

        code: String,

        #[unique]
        order_id: Option<ID>,

        #[belongs_to(key = order_id, references = id)]
        order: toasty::BelongsTo<Option<Order>>,

        #[has_many]
        line_items: toasty::HasMany<LineItem>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct LineItem {
        #[key]
        #[auto]
        id: ID,

        description: String,

        #[index]
        invoice_id: ID,

        #[belongs_to(key = invoice_id, references = id)]
        invoice: toasty::BelongsTo<Invoice>,
    }

    let mut db = test.setup_db(models!(Order, Invoice, LineItem)).await;

    let order = Order::create()
        .label("Order1")
        .invoice(
            Invoice::create()
                .code("INV-001")
                .line_item(LineItem::create().description("Widget"))
                .line_item(LineItem::create().description("Gadget")),
        )
        .exec(&mut db)
        .await?;

    let order = Order::filter_by_id(order.id)
        .include(Order::fields().invoice().line_items())
        .get(&mut db)
        .await?;

    let invoice = order.invoice.get();
    assert_eq!("INV-001", invoice.code);
    let mut descs: Vec<&str> = invoice
        .line_items
        .get()
        .iter()
        .map(|li| li.description.as_str())
        .collect();
    descs.sort();
    assert_eq!(descs, vec!["Gadget", "Widget"]);

    Ok(())
}

// ===== HasOne<Option<T>> -> HasOne<Option<T>> =====
// User has_one optional Profile, Profile has_one optional Avatar
#[ignore] // TODO: nested preload panics with type mismatch for HasOne<Option<T>> -> HasOne<Option<T>>
#[driver_test(id(ID))]
pub async fn nested_has_one_optional_then_has_one_optional(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        bio: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        #[has_one]
        avatar: toasty::HasOne<Option<Avatar>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Avatar {
        #[key]
        #[auto]
        id: ID,

        url: String,

        #[unique]
        profile_id: Option<ID>,

        #[belongs_to(key = profile_id, references = id)]
        profile: toasty::BelongsTo<Option<Profile>>,
    }

    let mut db = test.setup_db(models!(User, Profile, Avatar)).await;

    // User -> Profile -> Avatar (all present)
    let user = User::create()
        .name("Alice")
        .profile(
            Profile::create()
                .bio("hi")
                .avatar(Avatar::create().url("pic.png")),
        )
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().profile().avatar())
        .get(&mut db)
        .await?;

    let profile = user.profile.get().as_ref().unwrap();
    assert_eq!("hi", profile.bio);
    let avatar = profile.avatar.get().as_ref().unwrap();
    assert_eq!("pic.png", avatar.url);

    // User -> Profile (present) -> Avatar (missing)
    let user2 = User::create()
        .name("Bob")
        .profile(Profile::create().bio("no pic"))
        .exec(&mut db)
        .await?;

    let user2 = User::filter_by_id(user2.id)
        .include(User::fields().profile().avatar())
        .get(&mut db)
        .await?;

    let profile2 = user2.profile.get().as_ref().unwrap();
    assert_eq!("no pic", profile2.bio);
    assert!(profile2.avatar.get().is_none());

    // User -> Profile (missing) - nested preload short-circuits
    let user3 = User::create().name("Carol").exec(&mut db).await?;

    let user3 = User::filter_by_id(user3.id)
        .include(User::fields().profile().avatar())
        .get(&mut db)
        .await?;

    assert!(user3.profile.get().is_none());

    Ok(())
}

// ===== HasOne<T> (required) -> HasOne<T> (required) =====
// User has_one required Profile, Profile has_one required Avatar
#[ignore] // TODO: nested preload panics with type mismatch for HasOne<T> -> HasOne<T>
#[driver_test(id(ID))]
pub async fn nested_has_one_required_then_has_one_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Profile>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        bio: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        #[has_one]
        avatar: toasty::HasOne<Avatar>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Avatar {
        #[key]
        #[auto]
        id: ID,

        url: String,

        #[unique]
        profile_id: Option<ID>,

        #[belongs_to(key = profile_id, references = id)]
        profile: toasty::BelongsTo<Option<Profile>>,
    }

    let mut db = test.setup_db(models!(User, Profile, Avatar)).await;

    let user = User::create()
        .name("Alice")
        .profile(
            Profile::create()
                .bio("engineer")
                .avatar(Avatar::create().url("alice.jpg")),
        )
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().profile().avatar())
        .get(&mut db)
        .await?;

    let profile = user.profile.get();
    assert_eq!("engineer", profile.bio);
    let avatar = profile.avatar.get();
    assert_eq!("alice.jpg", avatar.url);

    Ok(())
}

// ===== HasOne<Option<T>> -> BelongsTo<T> (required) =====
// User has_one optional Review, Review belongs_to a Product
#[driver_test(id(ID))]
pub async fn nested_has_one_optional_then_belongs_to_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        review: toasty::HasOne<Option<Review>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Product {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Review {
        #[key]
        #[auto]
        id: ID,

        body: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        #[index]
        product_id: ID,

        #[belongs_to(key = product_id, references = id)]
        product: toasty::BelongsTo<Product>,
    }

    let mut db = test.setup_db(models!(User, Product, Review)).await;

    let product = Product::create().name("Widget").exec(&mut db).await?;

    let user = User::create()
        .name("Alice")
        .review(Review::create().body("Great!").product(&product))
        .exec(&mut db)
        .await?;

    // User with review -> preload nested product
    let user = User::filter_by_id(user.id)
        .include(User::fields().review().product())
        .get(&mut db)
        .await?;

    let review = user.review.get().as_ref().unwrap();
    assert_eq!("Great!", review.body);
    assert_eq!("Widget", review.product.get().name);

    // User without review
    let user2 = User::create().name("Bob").exec(&mut db).await?;

    let user2 = User::filter_by_id(user2.id)
        .include(User::fields().review().product())
        .get(&mut db)
        .await?;

    assert!(user2.review.get().is_none());

    Ok(())
}

// ===== BelongsTo<T> (required) -> HasMany =====
// Comment belongs_to a Post, Post has_many Tags
#[driver_test(id(ID))]
pub async fn nested_belongs_to_required_then_has_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[has_many]
        tags: toasty::HasMany<Tag>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Tag {
        #[key]
        #[auto]
        id: ID,

        label: String,

        #[index]
        post_id: ID,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::BelongsTo<Post>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Comment {
        #[key]
        #[auto]
        id: ID,

        body: String,

        #[index]
        post_id: ID,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::BelongsTo<Post>,
    }

    let mut db = test.setup_db(models!(Post, Tag, Comment)).await;

    let post = Post::create()
        .title("Hello")
        .tag(Tag::create().label("rust"))
        .tag(Tag::create().label("orm"))
        .exec(&mut db)
        .await?;

    let comment = Comment::create()
        .body("Nice post")
        .post(&post)
        .exec(&mut db)
        .await?;

    // From comment, preload post's tags
    let comment = Comment::filter_by_id(comment.id)
        .include(Comment::fields().post().tags())
        .get(&mut db)
        .await?;

    assert_eq!("Hello", comment.post.get().title);
    let mut labels: Vec<&str> = comment
        .post
        .get()
        .tags
        .get()
        .iter()
        .map(|t| t.label.as_str())
        .collect();
    labels.sort();
    assert_eq!(labels, vec!["orm", "rust"]);

    Ok(())
}

// ===== BelongsTo<T> (required) -> HasOne<Option<T>> =====
// Todo belongs_to a User, User has_one optional Profile
#[ignore] // TODO: nested preload panics with type mismatch for BelongsTo<T> -> HasOne<Option<T>>
#[driver_test(id(ID))]
pub async fn nested_belongs_to_required_then_has_one_optional(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        bio: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let mut db = test.setup_db(models!(User, Profile, Todo)).await;

    // User with profile
    let user = User::create()
        .name("Alice")
        .profile(Profile::create().bio("developer"))
        .todo(Todo::create().title("Task 1"))
        .exec(&mut db)
        .await?;

    let todo_id = Todo::filter_by_user_id(user.id)
        .first(&mut db)
        .await?
        .unwrap()
        .id;

    let todo = Todo::filter_by_id(todo_id)
        .include(Todo::fields().user().profile())
        .get(&mut db)
        .await?;

    assert_eq!("Alice", todo.user.get().name);
    let profile = todo.user.get().profile.get().as_ref().unwrap();
    assert_eq!("developer", profile.bio);

    // User without profile
    let user2 = User::create()
        .name("Bob")
        .todo(Todo::create().title("Task 2"))
        .exec(&mut db)
        .await?;

    let todo2_id = Todo::filter_by_user_id(user2.id)
        .first(&mut db)
        .await?
        .unwrap()
        .id;

    let todo2 = Todo::filter_by_id(todo2_id)
        .include(Todo::fields().user().profile())
        .get(&mut db)
        .await?;

    assert_eq!("Bob", todo2.user.get().name);
    assert!(todo2.user.get().profile.get().is_none());

    Ok(())
}

// ===== BelongsTo<T> (required) -> BelongsTo<T> (required) =====
// Step belongs_to a Todo, Todo belongs_to a User (chain of belongs_to going up)
#[driver_test(id(ID))]
pub async fn nested_belongs_to_required_then_belongs_to_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_many]
        steps: toasty::HasMany<Step>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Step {
        #[key]
        #[auto]
        id: ID,

        description: String,

        #[index]
        todo_id: ID,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::BelongsTo<Todo>,
    }

    let mut db = test.setup_db(models!(User, Todo, Step)).await;

    let user = User::create()
        .name("Alice")
        .todo(
            Todo::create()
                .title("T1")
                .step(Step::create().description("S1")),
        )
        .exec(&mut db)
        .await?;

    let todo_id = Todo::filter_by_user_id(user.id)
        .first(&mut db)
        .await?
        .unwrap()
        .id;
    let step_id = Step::filter_by_todo_id(todo_id)
        .first(&mut db)
        .await?
        .unwrap()
        .id;

    // From step, preload todo and then todo's user
    let step = Step::filter_by_id(step_id)
        .include(Step::fields().todo().user())
        .get(&mut db)
        .await?;

    assert_eq!("T1", step.todo.get().title);
    assert_eq!("Alice", step.todo.get().user.get().name);

    Ok(())
}

// ===== BelongsTo<Option<T>> -> HasMany =====
// Task optionally belongs_to a Project, Project has_many Members
#[driver_test(id(ID))]
pub async fn nested_belongs_to_optional_then_has_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Project {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        members: toasty::HasMany<Member>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Member {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[index]
        project_id: ID,

        #[belongs_to(key = project_id, references = id)]
        project: toasty::BelongsTo<Project>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Task {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        project_id: Option<ID>,

        #[belongs_to(key = project_id, references = id)]
        project: toasty::BelongsTo<Option<Project>>,
    }

    let mut db = test.setup_db(models!(Project, Member, Task)).await;

    let project = Project::create()
        .name("Proj1")
        .member(Member::create().name("Alice"))
        .member(Member::create().name("Bob"))
        .exec(&mut db)
        .await?;

    // Task with project
    let task = Task::create()
        .title("Linked")
        .project(&project)
        .exec(&mut db)
        .await?;

    let task = Task::filter_by_id(task.id)
        .include(Task::fields().project().members())
        .get(&mut db)
        .await?;

    let proj = task.project.get().as_ref().unwrap();
    assert_eq!("Proj1", proj.name);
    let mut names: Vec<&str> = proj.members.get().iter().map(|m| m.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["Alice", "Bob"]);

    // Task without project
    let orphan = Task::create().title("Orphan").exec(&mut db).await?;

    let orphan = Task::filter_by_id(orphan.id)
        .include(Task::fields().project().members())
        .get(&mut db)
        .await?;

    assert!(orphan.project.get().is_none());

    Ok(())
}

// ===== BelongsTo<Option<T>> -> BelongsTo<Option<T>> =====
// Comment optionally belongs_to a Post, Post optionally belongs_to a Category
#[ignore] // TODO: nested preload panics with type mismatch for BelongsTo<Option<T>> -> BelongsTo<Option<T>>
#[driver_test(id(ID))]
pub async fn nested_belongs_to_optional_then_belongs_to_optional(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Category {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        category_id: Option<ID>,

        #[belongs_to(key = category_id, references = id)]
        category: toasty::BelongsTo<Option<Category>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Comment {
        #[key]
        #[auto]
        id: ID,

        body: String,

        #[index]
        post_id: Option<ID>,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::BelongsTo<Option<Post>>,
    }

    let mut db = test.setup_db(models!(Category, Post, Comment)).await;

    let cat = Category::create().name("Tech").exec(&mut db).await?;
    let post = Post::create()
        .title("Hello")
        .category(&cat)
        .exec(&mut db)
        .await?;

    // Comment -> Post (present) -> Category (present)
    let c1 = Comment::create()
        .body("Nice")
        .post(&post)
        .exec(&mut db)
        .await?;

    let c1 = Comment::filter_by_id(c1.id)
        .include(Comment::fields().post().category())
        .get(&mut db)
        .await?;

    let loaded_post = c1.post.get().as_ref().unwrap();
    assert_eq!("Hello", loaded_post.title);
    let loaded_cat = loaded_post.category.get().as_ref().unwrap();
    assert_eq!("Tech", loaded_cat.name);

    // Post without category
    let post2 = Post::create().title("Uncategorized").exec(&mut db).await?;
    let c2 = Comment::create()
        .body("Hmm")
        .post(&post2)
        .exec(&mut db)
        .await?;

    let c2 = Comment::filter_by_id(c2.id)
        .include(Comment::fields().post().category())
        .get(&mut db)
        .await?;

    let loaded_post2 = c2.post.get().as_ref().unwrap();
    assert_eq!("Uncategorized", loaded_post2.title);
    assert!(loaded_post2.category.get().is_none());

    // Comment without post
    let c3 = Comment::create().body("Orphan").exec(&mut db).await?;

    let c3 = Comment::filter_by_id(c3.id)
        .include(Comment::fields().post().category())
        .get(&mut db)
        .await?;

    assert!(c3.post.get().is_none());

    Ok(())
}

// ===== BelongsTo<T> -> HasOne<T> (required) =====
// Todo belongs_to a User, User has_one required Config
#[driver_test(id(ID))]
pub async fn nested_belongs_to_required_then_has_one_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        config: toasty::HasOne<Config>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Config {
        #[key]
        #[auto]
        id: ID,

        theme: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let mut db = test.setup_db(models!(User, Config, Todo)).await;

    let user = User::create()
        .name("Alice")
        .config(Config::create().theme("dark"))
        .todo(Todo::create().title("Task"))
        .exec(&mut db)
        .await?;

    let todo_id = Todo::filter_by_user_id(user.id)
        .first(&mut db)
        .await?
        .unwrap()
        .id;

    let todo = Todo::filter_by_id(todo_id)
        .include(Todo::fields().user().config())
        .get(&mut db)
        .await?;

    assert_eq!("Alice", todo.user.get().name);
    assert_eq!("dark", todo.user.get().config.get().theme);

    Ok(())
}

// ===== HasMany -> HasMany (with empty nested collections) =====
// Ensures that when some parents have children and others don't, nested preload
// correctly assigns empty collections rather than panicking.
#[driver_test(id(ID))]
pub async fn nested_has_many_then_has_many_with_empty_leaves(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_many]
        steps: toasty::HasMany<Step>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Step {
        #[key]
        #[auto]
        id: ID,

        description: String,

        #[index]
        todo_id: ID,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::BelongsTo<Todo>,
    }

    let mut db = test.setup_db(models!(User, Todo, Step)).await;

    let user = User::create()
        .name("Alice")
        .todo(
            Todo::create()
                .title("With Steps")
                .step(Step::create().description("S1")),
        )
        .todo(Todo::create().title("No Steps")) // empty nested
        .exec(&mut db)
        .await
        .unwrap();

    let user = User::filter_by_id(user.id)
        .include(User::fields().todos().steps())
        .get(&mut db)
        .await
        .unwrap();

    let todos = user.todos.get();
    assert_eq!(2, todos.len());

    let mut total_steps = 0;
    for todo in todos {
        let steps = todo.steps.get();
        if todo.title == "With Steps" {
            assert_eq!(1, steps.len());
            assert_eq!("S1", steps[0].description);
        } else {
            assert_eq!(0, steps.len());
        }
        total_steps += steps.len();
    }
    assert_eq!(1, total_steps);
}
