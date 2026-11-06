pm.test("status is 200", () => pm.response.to.have.status(200));
pm.test("id matches the data row", () => {
  const id = pm.iterationData.get("user_id");
  if (id) pm.expect(pm.response.json().id).to.eql(Number(id));
});
pm.test("name matches the data row", () => {
  const name = pm.iterationData.get("name");
  if (name) pm.expect(pm.response.json().name).to.eql(name);
});
